import {
  AnnualPayrollParameters,
  BordroDonemi,
  BordroKaydi,
  CompensationRevision,
  CompensationRevisionOverride,
  DönemselKurumDegerleri,
  Personel,
  PersonelPuantaj,
  PersonelTaxOpening,
  PayrollAccrualInput,
  RetroAdjustmentBatch,
  RetroAllocation,
  SickLeaveRecord,
  ZAM_AYLARI_SETTING_KEY,
} from '../types/payroll';
import { RetroPreviewInput } from '../components/GeriyeDonukFarklar';
import { tauriBridge } from '../services/tauriBridge';
import {
  applyBrowserPayrollImpact,
  applyBrowserRetroBatchImpact,
  assertBrowserMutationImpactAllowed,
} from '../services/storage/browserPayrollPolicies';
import {
  MutationImpact,
  PayrollBoundaryPayroll,
  PayrollBoundaryPersonel,
  PayrollBoundaryTaxOpening,
  PayrollDatasetSnapshot,
  PayrollEngine,
  PayrollMutation,
  RetroCalculationRequest,
  RetroCalculationResult,
  RetroCalculationResultModel,
} from '../services/payrollEngine';
import {
  mergePayrollUiIntoBoundary,
  toPayrollBoundaryDto,
  toPayrollUiModel,
  type PayrollStorageDto,
} from '../services/payrollEngine/decimalBoundary';
import { nextPaymentSequence } from '../services/payrollEngine/paymentEventOrder';

export interface PayrollMutationControllerOptions {
  isNative: boolean;
  payrollEngine: PayrollEngine;
  payrollDataset: PayrollDatasetSnapshot;
  authoritativePayload: PayrollStorageDto | null;
  donemler: BordroDonemi[];
  bordrolar: BordroKaydi[];
  sickLeaveRecords: SickLeaveRecord[];
  compensationRevisions: CompensationRevision[];
  compensationRevisionOverrides: CompensationRevisionOverride[];
  retroBatches: RetroAdjustmentBatch[];
  updateAuthoritativePayload: (
    update: (current: PayrollStorageDto) => PayrollStorageDto
  ) => void;
  loadData: () => Promise<void>;
}

function normalizeZamAylari(months: number[]): number[] {
  return [...new Set(
    months.filter(
      (month) => Number.isInteger(month) && month >= 1 && month <= 12
    )
  )].sort((left, right) => left - right);
}

function sameRevisionDefinition(
  left: CompensationRevision | undefined,
  right: CompensationRevision
): boolean {
  return Boolean(left) &&
    left!.id === right.id &&
    left!.reason === right.reason &&
    left!.title === right.title &&
    left!.effectiveFrom === right.effectiveFrom &&
    left!.effectiveTo === right.effectiveTo &&
    left!.decisionDate === right.decisionDate &&
    left!.signedAt === right.signedAt &&
    left!.description === right.description &&
    left!.scope === right.scope &&
    JSON.stringify(left!.personnelIds ?? []) === JSON.stringify(right.personnelIds ?? []) &&
    left!.personnelGroup === right.personnelGroup;
}

function sameRevisionOverrides(
  left: CompensationRevisionOverride[],
  right: CompensationRevisionOverride[]
): boolean {
  if (left.length !== right.length) return false;
  return left.every((candidate) => right.some((other) =>
    candidate.id === other.id &&
    candidate.revisionId === other.revisionId &&
    candidate.parameter === other.parameter &&
    candidate.value === other.value &&
    candidate.personnelId === other.personnelId
  ));
}

type BoundaryInstitutionSettings = PayrollStorageDto['kurumDegerleriMap'][string];
type BoundaryStatutoryParameterSnapshot = NonNullable<
  BoundaryInstitutionSettings['statutoryParameterSnapshot']
>;

function captureStatutoryParameterSnapshot(
  settings: BoundaryInstitutionSettings | undefined
): BoundaryStatutoryParameterSnapshot | undefined {
  if (!settings || settings.statutoryParameterSnapshot) {
    return settings?.statutoryParameterSnapshot;
  }
  const gunlukYemekIstisnasiGV =
    settings.gunlukYemekIstisnasiGV ?? settings.gunlukYemekIstisnasiSGK;
  if (
    settings.gunlukAsgariUcret === undefined ||
    settings.sgkIsciOraniYuzde === undefined ||
    settings.issizlikIsciOraniYuzde === undefined ||
    settings.pekTavanKatsayisi === undefined ||
    settings.gunlukYemekIstisnasiSGK === undefined ||
    gunlukYemekIstisnasiGV === undefined
  ) {
    return undefined;
  }
  return {
    gunlukAsgariUcret: settings.gunlukAsgariUcret,
    sgkIsciOraniYuzde: settings.sgkIsciOraniYuzde,
    issizlikIsciOraniYuzde: settings.issizlikIsciOraniYuzde,
    pekTavanKatsayisi: settings.pekTavanKatsayisi,
    gunlukYemekIstisnasiSGK: settings.gunlukYemekIstisnasiSGK,
    gunlukYemekIstisnasiGV,
    statutoryParameterSegments: settings.statutoryParameterSegments ?? [],
  };
}

/**
 * Owns mutation authorization, native/browser branching, and retro settlement
 * orchestration. Formula and policy authority remain in payroll-core; this
 * controller only coordinates adapters and applies their returned impacts.
 */
export function usePayrollMutationController({
  isNative,
  payrollEngine,
  payrollDataset,
  authoritativePayload,
  donemler,
  bordrolar,
  sickLeaveRecords,
  compensationRevisions,
  compensationRevisionOverrides,
  retroBatches,
  updateAuthoritativePayload,
  loadData,
}: PayrollMutationControllerOptions) {
  const evaluateBrowserMutations = async (
    mutation: PayrollMutation | PayrollMutation[],
    dataset: PayrollDatasetSnapshot = payrollDataset
  ): Promise<MutationImpact> => {
    const mutations = Array.isArray(mutation) ? mutation : [mutation];
    const impacts = await Promise.all(
      mutations.map((item) => payrollEngine.evaluateMutationPolicy(item, dataset))
    );
    const affected = new Map<string, MutationImpact['affectedPayrolls'][number]>();
    const blocked = new Map<string, MutationImpact['blockedByFinalized'][number]>();
    const affectedRetroBatches = new Set<string>();
    const blockedByFinalizedRetroBatches = new Set<string>();
    for (const impact of impacts) {
      for (const key of impact.affectedPayrolls) {
        affected.set(`${key.personnelId}\u0000${key.periodId}\u0000${key.accrualId ?? ''}`, key);
      }
      for (const key of impact.blockedByFinalized) {
        blocked.set(`${key.personnelId}\u0000${key.periodId}\u0000${key.accrualId ?? ''}`, key);
      }
      for (const batchId of impact.affectedRetroBatches) affectedRetroBatches.add(batchId);
      for (const batchId of impact.blockedByFinalizedRetroBatches) {
        blockedByFinalizedRetroBatches.add(batchId);
      }
    }
    const merged = {
      affectedPayrolls: [...affected.values()],
      blockedByFinalized: [...blocked.values()],
      affectedRetroBatches: [...affectedRetroBatches],
      blockedByFinalizedRetroBatches: [...blockedByFinalizedRetroBatches],
    } satisfies MutationImpact;
    assertBrowserMutationImpactAllowed(merged);
    return merged;
  };

  const handleSavePersonel = async (newPersonel: Personel | PayrollBoundaryPersonel) => {
    if (isNative) {
      await tauriBridge.savePersonnel(toPayrollBoundaryDto(newPersonel) as unknown as Personel);
      await loadData();
      return;
    }
    const impact = await evaluateBrowserMutations({ kind: 'PERSON', personnelId: newPersonel.id });
    updateAuthoritativePayload((current) => {
      const existing = current.personeller.find((person) => person.id === newPersonel.id);
      const exactPersonel = mergePayrollUiIntoBoundary(existing, newPersonel);
      const personeller = existing
        ? current.personeller.map((person) =>
            person.id === newPersonel.id ? exactPersonel : person
          )
        : [...current.personeller, exactPersonel];
      return {
        ...current,
        personeller,
        bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact),
        retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
      };
    });
  };

  const handleDeletePersonel = async (personelId: string) => {
    if (isNative) {
      await tauriBridge.deletePersonnel(personelId);
      await loadData();
      return;
    }
    const impact = await evaluateBrowserMutations({ kind: 'PERSON', personnelId: personelId });
    if (impact.affectedRetroBatches.length > 0) {
      throw new Error(
        'Retro batch tarihçesi bulunan personel silinemez; audit ledger korunmalıdır.'
      );
    }
    updateAuthoritativePayload((current) => ({
      ...current,
      personeller: current.personeller.filter((person) => person.id !== personelId),
      bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact).filter(
        (payroll) => payroll.personelId !== personelId
      ),
      retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
      puantajlar: current.puantajlar.filter((attendance) => attendance.personelId !== personelId),
      taxOpenings: current.taxOpenings.filter((opening) => opening.personnelId !== personelId),
      sickLeaveRecords: current.sickLeaveRecords.filter((record) => record.personnelId !== personelId),
    }));
  };

  const handleCreateDonem = async (
    newDonem: BordroDonemi,
    kurumDegerleri: DönemselKurumDegerleri
  ) => {
    if (isNative) {
      await tauriBridge.savePeriodWithSettings(newDonem, kurumDegerleri);
      await loadData();
      return;
    }
    const existing = donemler.find((period) => period.id === newDonem.id);
    const positionMutations: PayrollMutation[] = [{
      kind: 'PERIOD_FROM_POSITION',
      startDate: newDonem.baslangicTarihi,
      taxYear: newDonem.taxYear,
      taxMonth: newDonem.taxMonth,
    }];
    if (existing) {
      positionMutations.push({ kind: 'PERIOD', periodId: newDonem.id });
      const causalFieldsChanged =
        existing.yil !== newDonem.yil ||
        existing.ay !== newDonem.ay ||
        existing.baslangicTarihi !== newDonem.baslangicTarihi ||
        existing.bitisTarihi !== newDonem.bitisTarihi ||
        existing.taxYear !== newDonem.taxYear ||
        existing.taxMonth !== newDonem.taxMonth;
      if (causalFieldsChanged) {
        positionMutations.push({
          kind: 'PERIOD_FROM_POSITION',
          startDate: existing.baslangicTarihi,
          taxYear: existing.taxYear,
          taxMonth: existing.taxMonth,
        });
      }
    }
    const impact = await evaluateBrowserMutations(positionMutations);
    updateAuthoritativePayload((current) => {
      const previousSettings = current.kurumDegerleriMap[newDonem.id];
      const exactPeriod = mergePayrollUiIntoBoundary(
        current.donemler.find((period) => period.id === newDonem.id),
        newDonem
      );
      const donemler = current.donemler.some((period) => period.id === newDonem.id)
        ? current.donemler.map((period) =>
            period.id === newDonem.id ? exactPeriod : period
          )
        : [...current.donemler, exactPeriod];
      const exactSettings = mergePayrollUiIntoBoundary(
        previousSettings,
        kurumDegerleri
      );
      const preservedSettings = previousSettings?.statutoryParameterSnapshot
        ? { ...exactSettings, statutoryParameterSnapshot: previousSettings.statutoryParameterSnapshot }
        : exactSettings;
      return {
        ...current,
        donemler,
        kurumDegerleriMap: { ...current.kurumDegerleriMap, [newDonem.id]: preservedSettings },
        bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact),
        retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
      };
    });
  };

  const handleSaveKurumDegerleri = async (settings: DönemselKurumDegerleri) => {
    if (isNative) {
      await tauriBridge.saveInstitutionSettings(settings);
      await loadData();
      return;
    }
    const impact = await evaluateBrowserMutations({ kind: 'PERIOD', periodId: settings.donemId });
    updateAuthoritativePayload((current) => ({
      ...current,
      kurumDegerleriMap: {
        ...current.kurumDegerleriMap,
        [settings.donemId]: (() => {
          const previousSettings = current.kurumDegerleriMap[settings.donemId];
          const exactSettings = mergePayrollUiIntoBoundary(previousSettings, settings);
          return previousSettings?.statutoryParameterSnapshot
            ? { ...exactSettings, statutoryParameterSnapshot: previousSettings.statutoryParameterSnapshot }
            : exactSettings;
        })(),
      },
      bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact),
      retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
    }));
  };

  const handleSavePuantaj = async (updatedPuantaj: PersonelPuantaj) => {
    if (isNative) {
      await tauriBridge.saveAttendance(updatedPuantaj);
      await loadData();
      return;
    }
    const impact = await evaluateBrowserMutations({
      kind: 'PERSON_PERIOD',
      personnelId: updatedPuantaj.personelId,
      periodId: updatedPuantaj.donemId,
    });
    updateAuthoritativePayload((current) => {
      const index = current.puantajlar.findIndex((attendance) => attendance.id === updatedPuantaj.id);
      const puantajlar = [...current.puantajlar];
      const exactAttendance = mergePayrollUiIntoBoundary(
        index < 0 ? undefined : current.puantajlar[index],
        updatedPuantaj
      );
      if (index < 0) puantajlar.push(exactAttendance);
      else puantajlar[index] = exactAttendance;
      return {
        ...current,
        puantajlar,
        bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact),
        retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
      };
    });
  };

  const handleSaveTaxOpening = async (
    opening: PersonelTaxOpening | PayrollBoundaryTaxOpening
  ) => {
    if (isNative) {
      await tauriBridge.saveTaxOpening(toPayrollBoundaryDto(opening) as unknown as PersonelTaxOpening);
      await loadData();
      return;
    }
    const impact = await evaluateBrowserMutations({
      kind: 'PERSON_TAX_YEAR',
      personnelId: opening.personnelId,
      taxYear: opening.year,
    });
    updateAuthoritativePayload((current) => {
      const index = current.taxOpenings.findIndex((item) => item.id === opening.id);
      const taxOpenings = [...current.taxOpenings];
      const exactOpening = mergePayrollUiIntoBoundary(
        index < 0 ? undefined : current.taxOpenings[index],
        opening
      );
      if (index < 0) taxOpenings.push(exactOpening);
      else taxOpenings[index] = exactOpening;
      return {
        ...current,
        taxOpenings,
        bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact),
      };
    });
  };

  const handleSaveSickLeaveRecord = async (record: SickLeaveRecord) => {
    if (isNative) {
      await tauriBridge.saveSickLeaveRecord(record);
      await loadData();
      return;
    }
    const existing = sickLeaveRecords.find((item) => item.id === record.id);
    const mutations: PayrollMutation[] = [
      { kind: 'PERSON_FROM_DATE', personnelId: record.personnelId, effectiveFrom: record.startDate },
    ];
    if (existing) {
      mutations.push({
        kind: 'PERSON_FROM_DATE',
        personnelId: existing.personnelId,
        effectiveFrom: existing.startDate,
      });
    }
    const impact = await evaluateBrowserMutations(mutations);
    updateAuthoritativePayload((current) => {
      const index = current.sickLeaveRecords.findIndex((item) => item.id === record.id);
      const nextRecords = [...current.sickLeaveRecords];
      if (index < 0) nextRecords.push(record);
      else nextRecords[index] = record;
      return {
        ...current,
        sickLeaveRecords: nextRecords,
        bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact),
        retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
      };
    });
  };

  const handleSaveAnnualPayrollParameters = async (parameters: AnnualPayrollParameters) => {
    if (isNative) {
      await tauriBridge.saveAnnualPayrollParameters(parameters);
      await loadData();
      return;
    }
    const impact = await evaluateBrowserMutations({ kind: 'TAX_YEAR', taxYear: parameters.year });
    updateAuthoritativePayload((current) => {
      const index = current.annualPayrollParameters.findIndex((item) => item.year === parameters.year);
      const annualPayrollParameters = [...current.annualPayrollParameters];
      const exactParameters = mergePayrollUiIntoBoundary(
        index < 0 ? undefined : current.annualPayrollParameters[index],
        parameters
      );
      if (index < 0) annualPayrollParameters.push(exactParameters);
      else annualPayrollParameters[index] = exactParameters;
      annualPayrollParameters.sort((left, right) => left.year - right.year);
      return {
        ...current,
        annualPayrollParameters,
        bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact),
      };
    });
  };

  const handleSaveZamAylari = async (months: number[]) => {
    const normalized = normalizeZamAylari(months);
    if (isNative) {
      await tauriBridge.setAppSetting(ZAM_AYLARI_SETTING_KEY, JSON.stringify(normalized));
      await loadData();
      return;
    }
    const impact = await evaluateBrowserMutations({ kind: 'ALL' });
    updateAuthoritativePayload((current) => ({
      ...current,
      zamAylari: normalized,
      bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact),
      retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
    }));
  };

  const handleDeleteSickLeaveRecord = async (id: string) => {
    if (isNative) {
      await tauriBridge.deleteSickLeaveRecord(id);
      await loadData();
      return;
    }
    const record = sickLeaveRecords.find((item) => item.id === id);
    const impact = record
      ? await evaluateBrowserMutations({
          kind: 'PERSON_FROM_DATE',
          personnelId: record.personnelId,
          effectiveFrom: record.startDate,
        })
      : null;
    updateAuthoritativePayload((current) => ({
      ...current,
      sickLeaveRecords: current.sickLeaveRecords.filter((item) => item.id !== id),
      bordrolar: record
        ? applyBrowserPayrollImpact(current.bordrolar, impact!)
        : current.bordrolar,
      retroBatches: record
        ? applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact!)
        : current.retroBatches,
    }));
  };

  const handleDeleteBordro = async (event: BordroKaydi) => {
    const eventId = event.accrualId || event.id;
    if (isNative) {
      await tauriBridge.deletePayrollAccrual(event.personelId, event.donemId, eventId);
      await loadData();
      return;
    }
    const impact = await evaluateBrowserMutations({
      kind: 'ACCRUAL_DELETE',
      personnelId: event.personelId,
      periodId: event.donemId,
      accrualId: eventId,
    });
    updateAuthoritativePayload((current) => ({
      ...current,
      bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact)
        .filter((item) => (item.accrualId || item.id) !== eventId),
      retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
    }));
  };

  const handleSaveBordro = async (updatedBordro: PayrollBoundaryPayroll) => {
    if (isNative) {
      // Native persistence already invalidates downstream calculated rows in
      // one transaction; re-fetch the whole ledger for the UI.
      await loadData();
      return;
    }
    const existing = bordrolar.find(
      (payroll) => payroll.id === updatedBordro.id || payroll.accrualId === updatedBordro.accrualId
    );
    const mutation: PayrollMutation = existing
      ? {
          kind: 'ACCRUAL_CALCULATION',
          personnelId: updatedBordro.personelId,
          periodId: updatedBordro.donemId,
          accrualId: updatedBordro.accrualId,
        }
      : {
          kind: 'ACCRUAL_INSERT',
          personnelId: updatedBordro.personelId,
          periodId: updatedBordro.donemId,
          accrualId: updatedBordro.accrualId,
          paymentDate: updatedBordro.paymentDate,
          sequence: updatedBordro.sequence,
        };
    const impact = await evaluateBrowserMutations(mutation);
    updateAuthoritativePayload((current) => {
      const currentSettings = current.kurumDegerleriMap[updatedBordro.donemId];
      const capturedSnapshot = !currentSettings?.statutoryParameterSnapshot
        ? captureStatutoryParameterSnapshot(currentSettings)
        : undefined;
      const kurumDegerleriMap = capturedSnapshot && currentSettings
        ? {
            ...current.kurumDegerleriMap,
            [updatedBordro.donemId]: {
              ...currentSettings,
              statutoryParameterSnapshot: capturedSnapshot,
            },
          }
        : current.kurumDegerleriMap;
      const invalidated = applyBrowserPayrollImpact(current.bordrolar, impact);
      const index = invalidated.findIndex(
        (payroll) => payroll.id === updatedBordro.id || payroll.accrualId === updatedBordro.accrualId
      );
      if (index < 0) {
        return {
          ...current,
          kurumDegerleriMap,
          bordrolar: [...invalidated, updatedBordro],
          retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
        };
      }
      const next = [...invalidated];
      next[index] = updatedBordro;
      return {
        ...current,
        kurumDegerleriMap,
        bordrolar: next,
        retroBatches: updatedBordro.accrualType === 'RETRO_ADJUSTMENT' && updatedBordro.status === 'FINALIZED'
          ? (current.retroBatches ?? []).map((batch) =>
              batch.id === updatedBordro.accrualId
                ? {
                    ...batch,
                    status: 'FINALIZED' as const,
                    settlementStatus: 'PAID' as const,
                    finalizedAt: updatedBordro.sonGuncellemeTarihi,
                  }
                : batch
            )
          : current.retroBatches,
      };
    });
  };

  const handleSaveCompensationRevision = async (
    revision: CompensationRevision,
    overrides: CompensationRevisionOverride[]
  ) => {
    const existing = compensationRevisions.find((item) => item.id === revision.id);
    const revisionDefinitionChanged = !sameRevisionDefinition(existing, revision) ||
      !sameRevisionOverrides(
        compensationRevisionOverrides.filter((item) => item.revisionId === revision.id),
        overrides
      );
    if (existing?.status === 'FINALIZED') {
      if (revisionDefinitionChanged) {
        throw new Error('FINALIZED revision veya ona bağlı FINALIZED retro payment event’i değiştirilemez.');
      }
      return;
    }
    if (revisionDefinitionChanged && retroBatches.some(
      (batch) => batch.revisionId === revision.id && batch.status === 'FINALIZED'
    )) {
      throw new Error('FINALIZED revision veya ona bağlı FINALIZED retro payment event’i değiştirilemez.');
    }
    if (revision.status !== 'DRAFT') {
      throw new Error('Yeni veya değiştirilen compensation revision yalnızca DRAFT olabilir.');
    }
    const overrideKeys = new Set<string>();
    for (const override of overrides) {
      const key = `${override.parameter}\u0000${override.personnelId ?? ''}`;
      if (override.revisionId !== revision.id || overrideKeys.has(key)) {
        throw new Error('Revision override kayıtlarında duplicate veya yanlış revision ilişkisi var.');
      }
      overrideKeys.add(key);
    }

    if (isNative) {
      await tauriBridge.saveCompensationRevision(revision, overrides);
      await loadData();
      return;
    }

    const retroMutations: PayrollMutation[] = revisionDefinitionChanged
      ? bordrolar
          .filter((payroll) =>
            payroll.accrualType === 'RETRO_ADJUSTMENT' &&
            retroBatches.some(
              (batch) => batch.revisionId === revision.id &&
                batch.id === payroll.accrualId && batch.status !== 'FINALIZED'
            )
          )
          .map((payroll) => ({
            kind: 'ACCRUAL_CALCULATION' as const,
            personnelId: payroll.personelId,
            periodId: payroll.donemId,
            accrualId: payroll.accrualId,
          }))
      : [];
    const retroImpact = retroMutations.length
      ? await evaluateBrowserMutations(retroMutations)
      : null;
    const exactRevision = toPayrollBoundaryDto(revision) as unknown as NonNullable<PayrollStorageDto['compensationRevisions']>[number];
    const exactOverrides = toPayrollBoundaryDto(overrides) as unknown as NonNullable<PayrollStorageDto['compensationRevisionOverrides']>;
    updateAuthoritativePayload((current) => ({
      ...current,
      compensationRevisions: [
        ...(current.compensationRevisions ?? []).filter((item) => item.id !== revision.id),
        exactRevision,
      ],
      compensationRevisionOverrides: [
        ...(current.compensationRevisionOverrides ?? []).filter((item) => item.revisionId !== revision.id),
        ...exactOverrides,
      ],
      retroBatches: revisionDefinitionChanged
        ? (current.retroBatches ?? []).map((batch) =>
            batch.revisionId === revision.id && batch.status !== 'FINALIZED'
              ? { ...batch, status: 'STALE' as const }
              : batch
          )
        : current.retroBatches,
      bordrolar: applyBrowserPayrollImpact(current.bordrolar, retroImpact ?? {
        affectedPayrolls: [],
        blockedByFinalized: [],
        affectedRetroBatches: [],
        blockedByFinalizedRetroBatches: [],
      }).map((payroll) =>
        revisionDefinitionChanged && (current.retroBatches ?? []).some(
          (batch) => batch.revisionId === revision.id && batch.id === payroll.accrualId && batch.status !== 'FINALIZED'
        ) && payroll.status !== 'FINALIZED'
          ? { ...payroll, status: 'STALE' as const }
          : payroll
      ),
    }));
  };

  const handleCalculateRetroPreview = async (
    request: RetroPreviewInput
  ): Promise<RetroCalculationResultModel> => {
    const datasetForPreview: PayrollDatasetSnapshot = {
      ...payrollDataset,
      retroBatches: payrollDataset.retroBatches.map((batch) =>
        batch.id === request.batchId && batch.status !== 'FINALIZED'
          ? { ...batch, status: 'STALE' as const }
          : batch
      ),
    };
    const exactRequest = toPayrollBoundaryDto({
      ...request,
      dataset: datasetForPreview,
    }) as unknown as RetroCalculationRequest;
    const result: RetroCalculationResult = await payrollEngine.calculateRetroPreview(exactRequest);
    return toPayrollUiModel(result) as unknown as RetroCalculationResultModel;
  };

  const canonicalizeRetroResult = async (
    result: RetroCalculationResultModel
  ): Promise<RetroCalculationResultModel> => {
    const submittedBatch = result.batch;
    const existingBatch = payrollDataset.retroBatches.find((batch) => batch.id === submittedBatch.id);
    if (existingBatch && (
      existingBatch.revisionId !== submittedBatch.revisionId ||
      existingBatch.personnelId !== submittedBatch.personnelId ||
      existingBatch.paymentDate !== submittedBatch.paymentDate
    )) {
      throw new Error('Retro batch primary id’si farklı revision/personel/ödeme olayına ait; yeniden bağlanamaz.');
    }
    if (existingBatch?.status === 'FINALIZED') {
      throw new Error('FINALIZED retro batch yeniden hesaplanamaz; yeni bir correction batch’i oluşturulmalıdır.');
    }
    const existingPayrollWithBatchIdentity = payrollDataset.payrolls.find(
      (payroll) => payroll.accrualId === submittedBatch.id || payroll.id === submittedBatch.id
    );
    if (existingPayrollWithBatchIdentity && (
      existingPayrollWithBatchIdentity.personelId !== submittedBatch.personnelId ||
      existingPayrollWithBatchIdentity.accrualType !== 'RETRO_ADJUSTMENT' ||
      existingPayrollWithBatchIdentity.paymentDate !== submittedBatch.paymentDate
    )) {
      throw new Error('Retro batch kimliği mevcut bir farklı ödeme olayının kimliğiyle çakışıyor; yeni bir kimlik kullanın.');
    }
    const persistedRevision = compensationRevisions.find(
      (revision) => revision.id === submittedBatch.revisionId
    );
    if (!persistedRevision) {
      throw new Error(`Retro revision persisted dataset'te bulunamadı: ${submittedBatch.revisionId}`);
    }
    const replayDataset: PayrollDatasetSnapshot = {
      ...payrollDataset,
      retroBatches: payrollDataset.retroBatches.filter((item) => item.id !== submittedBatch.id),
      retroAllocations: payrollDataset.retroAllocations.filter(
        (item) => item.batchId !== submittedBatch.id
      ),
    };
    const canonicalRequest = toPayrollBoundaryDto({
      batchId: submittedBatch.id,
      revision: persistedRevision,
      overrides: compensationRevisionOverrides.filter(
        (item) => item.revisionId === persistedRevision.id
      ),
      personnelId: submittedBatch.personnelId,
      paymentDate: submittedBatch.paymentDate,
      calculatedAt: submittedBatch.calculatedAt || submittedBatch.createdAt || new Date().toISOString(),
      description: submittedBatch.description || null,
      dataset: replayDataset,
    }) as unknown as RetroCalculationRequest;
    const canonicalResult = toPayrollUiModel(
      await payrollEngine.calculateRetroPreview(canonicalRequest)
    ) as unknown as RetroCalculationResultModel;
    const sortAllocations = (allocations: RetroAllocation[]) =>
      [...allocations].sort((left, right) => left.id.localeCompare(right.id));
    const batchFinancialSnapshot = (batch: RetroAdjustmentBatch) => [
      batch.totalGrossDelta,
      batch.payableSettlementAmount ?? 0,
      batch.offsetSettlementAmount ?? 0,
      batch.recoveredAmount ?? 0,
      batch.recoverableAmount ?? 0,
      batch.outstandingReceivable ?? 0,
      batch.settlementStatus ?? null,
    ];
    if (
      JSON.stringify(batchFinancialSnapshot(canonicalResult.batch)) !==
        JSON.stringify(batchFinancialSnapshot(submittedBatch)) ||
      JSON.stringify(toPayrollBoundaryDto(sortAllocations(canonicalResult.allocations))) !==
        JSON.stringify(toPayrollBoundaryDto(sortAllocations(result.allocations)))
    ) {
      throw new Error('Retro preview güncel veriyle eşleşmiyor; yeniden hesaplayın.');
    }
    return canonicalResult;
  };

  const handleSaveRetroBatch = async (result: RetroCalculationResultModel) => {
    const canonicalResult = await canonicalizeRetroResult(result);
    const batch = canonicalResult.batch;
    if (
      batch.payableSettlementAmount !== 0 ||
      !['OVERPAYMENT', 'SETTLED_BY_OFFSET'].includes(batch.settlementStatus ?? '')
    ) {
      throw new Error('Payment event olmadan yalnız açık fazla tahakkuk veya mahsupla kapanan settlement batch’i saklanabilir.');
    }
    const activePayment = payrollDataset.payrolls.find(
      (payroll) => payroll.accrualId === batch.id &&
        (payroll.status === 'DRAFT' || payroll.status === 'CALCULATED' || payroll.status === 'FINALIZED')
    );
    if (activePayment) {
      throw new Error(`${batch.id} retro payment event'i ${activePayment.status} durumunda; event silinmeden fazla tahakkuk batch'i saklanamaz.`);
    }
    if (isNative) {
      await tauriBridge.saveRetroAdjustmentBatch(batch, canonicalResult.allocations);
      await loadData();
      return;
    }
    if (!authoritativePayload) throw new Error('Yetkili veri snapshot’ı hazır değil.');
    const exactBatch = toPayrollBoundaryDto(batch) as unknown as NonNullable<PayrollStorageDto['retroBatches']>[number];
    const exactAllocations = toPayrollBoundaryDto(canonicalResult.allocations) as unknown as NonNullable<PayrollStorageDto['retroAllocations']>;
    const datasetWithBatch: PayrollDatasetSnapshot = {
      ...payrollDataset,
      retroBatches: [...payrollDataset.retroBatches.filter((item) => item.id !== batch.id), exactBatch],
      retroAllocations: [
        ...payrollDataset.retroAllocations.filter((item) => item.batchId !== batch.id),
        ...exactAllocations,
      ],
    };
    const impact = await evaluateBrowserMutations(
      {
        kind: 'RETRO_BATCH_SAVE',
        personnelId: batch.personnelId,
        batchId: batch.id,
        paymentDate: batch.paymentDate,
      },
      datasetWithBatch
    );
    updateAuthoritativePayload((current) => ({
      ...current,
      compensationRevisions: current.compensationRevisions ?? [],
      compensationRevisionOverrides: current.compensationRevisionOverrides ?? [],
      retroBatches: [...(current.retroBatches ?? []).filter((item) => item.id !== batch.id), exactBatch],
      retroAllocations: [
        ...(current.retroAllocations ?? []).filter((item) => item.batchId !== batch.id),
        ...exactAllocations,
      ],
      bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact),
    }));
  };

  const handleCreateRetroPayment = async (result: RetroCalculationResultModel) => {
    const canonicalResult = await canonicalizeRetroResult(result);
    const batch = canonicalResult.batch;
    if (batch.payableSettlementAmount <= 0) {
      throw new Error('Payable settlement sıfır; entitlement yalnız settlement ledger’ında kalır ve payment event oluşturulmaz.');
    }
    const paymentParts = batch.paymentDate.split('-').map(Number);
    const paymentPeriod = donemler.find(
      (period) => period.taxYear === paymentParts[0] && period.taxMonth === paymentParts[1]
    );
    if (!paymentPeriod) {
      throw new Error(`${batch.paymentDate.slice(0, 7)} için payment/tax period bulunamadı.`);
    }
    const existingPayment = payrollDataset.payrolls.find(
      (payroll) => payroll.personelId === batch.personnelId &&
        payroll.donemId === paymentPeriod.id && payroll.accrualId === batch.id
    );
    const sequence = existingPayment?.sequence ??
      nextPaymentSequence(payrollDataset, batch.personnelId, paymentPeriod, batch.paymentDate);
    const accrual: PayrollAccrualInput = {
      accrualId: batch.id,
      accrualType: 'RETRO_ADJUSTMENT',
      paymentDate: batch.paymentDate,
      sequence,
      grossAmount: batch.payableSettlementAmount,
      description: batch.description || 'Geriye dönük hakediş farkı',
    };

    if (isNative) {
      await tauriBridge.createRetroPayment(
        batch,
        canonicalResult.allocations,
        paymentPeriod.id,
        sequence
      );
      await loadData();
      return;
    }
    if (!authoritativePayload) throw new Error('Yetkili veri snapshot’ı hazır değil.');
    const exactBatch = toPayrollBoundaryDto(batch) as unknown as NonNullable<PayrollStorageDto['retroBatches']>[number];
    const exactAllocations = toPayrollBoundaryDto(canonicalResult.allocations) as unknown as NonNullable<PayrollStorageDto['retroAllocations']>;
    const datasetWithBatch: PayrollDatasetSnapshot = {
      ...payrollDataset,
      retroBatches: [...payrollDataset.retroBatches.filter((item) => item.id !== batch.id), exactBatch],
      retroAllocations: [
        ...payrollDataset.retroAllocations.filter((item) => item.batchId !== batch.id),
        ...exactAllocations,
      ],
    };
    const mutation: PayrollMutation = existingPayment
      ? {
          kind: 'ACCRUAL_CALCULATION',
          personnelId: batch.personnelId,
          periodId: paymentPeriod.id,
          accrualId: batch.id,
        }
      : {
          kind: 'ACCRUAL_INSERT',
          personnelId: batch.personnelId,
          periodId: paymentPeriod.id,
          accrualId: batch.id,
          paymentDate: batch.paymentDate,
          sequence,
        };
    const impact = await evaluateBrowserMutations(
      [
        {
          kind: 'RETRO_BATCH_SAVE',
          personnelId: batch.personnelId,
          batchId: batch.id,
          paymentDate: batch.paymentDate,
        },
        mutation,
      ],
      datasetWithBatch
    );
    const paymentRequest = {
      personnelId: batch.personnelId,
      periodId: paymentPeriod.id,
      calculatedAt: new Date().toISOString(),
      manualIncome: null,
      accrual: toPayrollBoundaryDto(accrual) as unknown as Parameters<typeof payrollEngine.calculatePayroll>[0]['accrual'],
      dataset: datasetWithBatch,
    };
    await payrollEngine.validatePayroll(paymentRequest);
    const calculated = await payrollEngine.calculatePayroll(paymentRequest);
    updateAuthoritativePayload((current) => {
      const invalidated = applyBrowserPayrollImpact(current.bordrolar, impact);
      const existingPayrollIndex = invalidated.findIndex(
        (item) => item.accrualId === batch.id || item.id === batch.id
      );
      const nextPayrolls = [...invalidated];
      if (existingPayrollIndex < 0) nextPayrolls.push(calculated);
      else nextPayrolls[existingPayrollIndex] = calculated;
      return {
        ...current,
        compensationRevisions: current.compensationRevisions ?? [],
        compensationRevisionOverrides: current.compensationRevisionOverrides ?? [],
        retroBatches: [...(current.retroBatches ?? []).filter((item) => item.id !== batch.id), exactBatch],
        retroAllocations: [
          ...(current.retroAllocations ?? []).filter((item) => item.batchId !== batch.id),
          ...exactAllocations,
        ],
        bordrolar: nextPayrolls,
      };
    });
  };

  return {
    evaluateBrowserMutations,
    handleSavePersonel,
    handleDeletePersonel,
    handleCreateDonem,
    handleSaveKurumDegerleri,
    handleSavePuantaj,
    handleSaveTaxOpening,
    handleSaveSickLeaveRecord,
    handleSaveAnnualPayrollParameters,
    handleSaveZamAylari,
    handleDeleteSickLeaveRecord,
    handleDeleteBordro,
    handleSaveBordro,
    handleSaveCompensationRevision,
    handleCalculateRetroPreview,
    handleSaveRetroBatch,
    handleCreateRetroPayment,
  };
}
