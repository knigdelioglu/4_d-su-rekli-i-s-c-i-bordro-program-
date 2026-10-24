import { useLayoutEffect, useRef } from 'react';
import {
  AnnualPayrollParameters,
  BordroDonemi,
  BordroKaydi,
  CompensationRevision,
  CompensationRevisionOverride,
  PayrollPeriodInstitutionSettings,
  Personel,
  PersonelPuantaj,
  PersonelTaxOpening,
  PayrollAccrualInput,
  RetroAdjustmentBatch,
  RetroAllocation,
  SickLeaveRecord,
} from '../types/payroll';
import { RetroPreviewInput } from '../components/GeriyeDonukFarklar';
import { tauriBridge } from '../services/tauriBridge';
import {
  applyBrowserPayrollImpact,
  applyBrowserRetroBatchImpact,
  assertBrowserMutationImpactAllowed,
  personnelHasFinancialChanges,
  taxOpeningHasFinancialChanges,
  validatePayrollStatusTransition,
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
import { assertPayrollCalculationSnapshotCurrent } from '../services/payrollEngine/calculationSnapshot';
import { sameSettingsValue } from '../components/Settings/periodSettingsSave';
import { deletePeriodFromPayload } from '../services/storage/periodDeletion';
import { reconcileStatutorySnapshot } from '../services/storage/statutorySnapshotPolicy';
import { getDefaultAnnualPayrollParameters } from '../services/storage/payrollDefaults';
import {
  isValidIsoDate,
  validatePuantaj,
  validateSickLeaveRecord,
} from '../services/storage/payrollPayloadSchema';
import {
  syncPuantajForSickLeaveDelete,
  syncPuantajForSickLeaveSave,
} from '../utils/sickLeaveSync';
import { createPuantajBulkPlan, type PuantajBulkCommitIntent } from '../utils/puantajBulkPlan';
import {
  assertUniqueTcNo,
  preparePersonnelForSave,
} from '../utils/personnelValidation';

export interface PayrollMutationControllerOptions {
  isNative: boolean;
  payrollEngine: PayrollEngine;
  payrollDataset: PayrollDatasetSnapshot;
  authoritativePayload: PayrollStorageDto | null;
  donemler: BordroDonemi[];
  puantajlar: PersonelPuantaj[];
  bordrolar: BordroKaydi[];
  sickLeaveRecords: SickLeaveRecord[];
  compensationRevisions: CompensationRevision[];
  compensationRevisionOverrides: CompensationRevisionOverride[];
  retroBatches: RetroAdjustmentBatch[];
  updateAuthoritativePayload: (
    update: (current: PayrollStorageDto) => PayrollStorageDto
  ) => void;
  commitAuthoritativePayload?: (
    update: (current: PayrollStorageDto) => PayrollStorageDto | Promise<PayrollStorageDto>
  ) => Promise<void>;
  loadData: () => Promise<void>;
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

function payrollDatasetFromPayload(payload: PayrollStorageDto): PayrollDatasetSnapshot {
  return {
    personnel: payload.personeller,
    periods: payload.donemler,
    institutionSettings: payload.kurumDegerleriMap,
    attendances: payload.puantajlar,
    payrolls: payload.bordrolar,
    taxOpenings: payload.taxOpenings,
    sickLeaveRecords: payload.sickLeaveRecords,
    annualPayrollParameters: payload.annualPayrollParameters,
    compensationRevisions: payload.compensationRevisions,
    compensationRevisionOverrides: payload.compensationRevisionOverrides,
    retroBatches: payload.retroBatches,
    retroAllocations: payload.retroAllocations,
    compensationPolicyVersions: payload.compensationPolicyVersions,
  } as unknown as PayrollDatasetSnapshot;
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

function ensureBoundaryAnnualPayrollParameters(
  parameters: PayrollStorageDto['annualPayrollParameters'],
  year: number
): PayrollStorageDto['annualPayrollParameters'] {
  if (parameters.some((parameter) => parameter.year === year)) return parameters;
  const defaults = getDefaultAnnualPayrollParameters(year);
  if (!defaults) return parameters;
  return [...parameters, toPayrollBoundaryDto(defaults)].sort(
    (left, right) => left.year - right.year
  );
}

export function mergeBrowserTaxOpening(
  openings: PayrollStorageDto['taxOpenings'],
  opening: PersonelTaxOpening | PayrollBoundaryTaxOpening
): PayrollStorageDto['taxOpenings'] {
  const index = openings.findIndex(
    (item) =>
      item.id === opening.id ||
      (item.personnelId === opening.personnelId && item.year === opening.year)
  );
  const nextOpenings = [...openings];
  const previousOpening = index < 0 ? undefined : openings[index];
  const clearNormalOpening =
    opening.gvCumulativeOpening === null || opening.effectiveFromPeriodId === null;
  const clearAsgariOpening =
    opening.asgariGvCumulativeOpening === null || opening.asgariGvEffectiveFromPeriodId === null;
  const mergedOpening = { ...opening } as PayrollBoundaryTaxOpening;

  if (clearNormalOpening) {
    mergedOpening.gvCumulativeOpening = null;
    mergedOpening.effectiveFromPeriodId = null;
  } else if (
    opening.gvCumulativeOpening === undefined &&
    previousOpening?.gvCumulativeOpening !== undefined
  ) {
    mergedOpening.gvCumulativeOpening = previousOpening.gvCumulativeOpening;
    mergedOpening.effectiveFromPeriodId = previousOpening.effectiveFromPeriodId;
  }
  if (clearAsgariOpening) {
    mergedOpening.asgariGvCumulativeOpening = null;
    mergedOpening.asgariGvEffectiveFromPeriodId = null;
  } else if (
    opening.asgariGvCumulativeOpening === undefined &&
    previousOpening?.asgariGvCumulativeOpening !== undefined
  ) {
    mergedOpening.asgariGvCumulativeOpening = previousOpening.asgariGvCumulativeOpening;
    mergedOpening.asgariGvEffectiveFromPeriodId = previousOpening.asgariGvEffectiveFromPeriodId;
  }

  const hasNormalValue = mergedOpening.gvCumulativeOpening != null;
  const hasNormalPeriod = mergedOpening.effectiveFromPeriodId != null;
  const hasAsgariValue = mergedOpening.asgariGvCumulativeOpening != null;
  const hasAsgariPeriod = mergedOpening.asgariGvEffectiveFromPeriodId != null;
  if (hasNormalValue !== hasNormalPeriod || hasAsgariValue !== hasAsgariPeriod) {
    throw new Error('Vergi opening bileşenleri değer ve effective dönem çiftleri olarak birlikte tanımlanmalıdır.');
  }

  const exactOpening = mergePayrollUiIntoBoundary(previousOpening, mergedOpening);
  if (index < 0) nextOpenings.push(exactOpening);
  else nextOpenings[index] = exactOpening;
  return nextOpenings;
}

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
  puantajlar,
  bordrolar,
  sickLeaveRecords,
  compensationRevisions,
  compensationRevisionOverrides,
  retroBatches,
  updateAuthoritativePayload,
  commitAuthoritativePayload,
  loadData,
}: PayrollMutationControllerOptions) {
  const latestPayrollDataset = useRef(payrollDataset);
  useLayoutEffect(() => {
    latestPayrollDataset.current = payrollDataset;
  }, [payrollDataset]);

  const evaluateBrowserMutations = async (
    mutation: PayrollMutation | PayrollMutation[],
    dataset?: PayrollDatasetSnapshot
  ): Promise<MutationImpact> => {
    const mutations = Array.isArray(mutation) ? mutation : [mutation];
    const policyDataset = dataset ?? latestPayrollDataset.current;
    const impacts = await Promise.all(
      mutations.map((item) => payrollEngine.evaluateMutationPolicy(item, policyDataset))
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
    assertBrowserMutationImpactAllowed(merged, {
      person: (id) => {
        const person = policyDataset.personnel.find((item) => item.id === id);
        return person ? `${person.ad} ${person.soyad}` : undefined;
      },
      period: (id) => policyDataset.periods.find((item) => item.id === id)?.donemAdi,
    });
    return merged;
  };

  const validatePuantajForSave = (attendance: PersonelPuantaj) => {
    validatePuantaj(attendance);
    const period = donemler.find((item) => item.id === attendance.donemId);
    if (!period) return;

    if (isValidIsoDate(period.baslangicTarihi) && isValidIsoDate(period.bitisTarihi)) {
      const [sy, sm, sd] = period.baslangicTarihi.split('-').map(Number);
      const [ey, em, ed] = period.bitisTarihi.split('-').map(Number);
      const startMs = Date.UTC(sy, sm - 1, sd);
      const endMs = Date.UTC(ey, em - 1, ed);
      const calendarDayCount = Math.round((endMs - startMs) / 86400000) + 1;
      const keys = Object.keys(attendance.gunler);
      if (keys.length > calendarDayCount) {
        throw new Error(
          `${period.id} dönemi ${calendarDayCount} takvim günü içeriyor ancak puantajda ${keys.length} kayıt var.`
        );
      }
    }
    for (const dateText of Object.keys(attendance.gunler)) {
      if (dateText < period.baslangicTarihi || dateText > period.bitisTarihi) {
        throw new Error(
          `${dateText} puantaj tarihi ${period.id} döneminin ${period.baslangicTarihi}–${period.bitisTarihi} aralığı dışında.`
        );
      }
    }
  };

  const handleSavePersonel = async (newPersonel: Personel | PayrollBoundaryPersonel) => {
    const preparedPersonel = preparePersonnelForSave(newPersonel);
    if (isNative) {
      await tauriBridge.savePersonnel(toPayrollBoundaryDto(preparedPersonel) as unknown as Personel);
      await loadData();
      return;
    }

    if (!authoritativePayload) throw new Error('Personel kaydedilemedi: yetkili veri snapshotı bulunamadı.');
    assertUniqueTcNo(authoritativePayload.personeller, preparedPersonel);
    if (commitAuthoritativePayload) {
      await commitAuthoritativePayload(async (current) => {
        assertUniqueTcNo(current.personeller, preparedPersonel);
        const existing = current.personeller.find((person) => person.id === preparedPersonel.id);
        const hasFinancialChange =
          !existing || personnelHasFinancialChanges(existing, preparedPersonel as Personel);
        const impact = hasFinancialChange
          ? await evaluateBrowserMutations(
              { kind: 'PERSON', personnelId: preparedPersonel.id },
              payrollDatasetFromPayload(current)
            )
          : undefined;
        const existingInCurrent = current.personeller.find((person) => person.id === preparedPersonel.id);
        const exactPersonel = mergePayrollUiIntoBoundary(existingInCurrent, preparedPersonel);
        const personeller = existingInCurrent
          ? current.personeller.map((person) =>
              person.id === preparedPersonel.id ? exactPersonel : person
            )
          : [...current.personeller, exactPersonel];
        return {
          ...current,
          personeller,
          bordrolar: impact ? applyBrowserPayrollImpact(current.bordrolar, impact) : current.bordrolar,
          retroBatches: impact
            ? applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact)
            : current.retroBatches,
        };
      });
      return;
    }

    const existing = authoritativePayload.personeller.find((person) => person.id === preparedPersonel.id);
    const hasFinancialChange =
      !existing || personnelHasFinancialChanges(existing, preparedPersonel as Personel);
    const impact = hasFinancialChange
      ? await evaluateBrowserMutations({ kind: 'PERSON', personnelId: preparedPersonel.id })
      : undefined;
    updateAuthoritativePayload((current) => {
      assertUniqueTcNo(current.personeller, preparedPersonel);
      const existingInCurrent = current.personeller.find((person) => person.id === preparedPersonel.id);
      const exactPersonel = mergePayrollUiIntoBoundary(existingInCurrent, preparedPersonel);
      const personeller = existingInCurrent
        ? current.personeller.map((person) =>
            person.id === preparedPersonel.id ? exactPersonel : person
          )
        : [...current.personeller, exactPersonel];
      return {
        ...current,
        personeller,
        bordrolar: impact ? applyBrowserPayrollImpact(current.bordrolar, impact) : current.bordrolar,
        retroBatches: impact ? applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact) : current.retroBatches,
      };
    });
  };

  const handleSavePersonelAndTaxOpening = async (
    newPersonel: Personel | PayrollBoundaryPersonel,
    opening: PersonelTaxOpening | PayrollBoundaryTaxOpening
  ) => {
    const preparedPersonel = preparePersonnelForSave(newPersonel);
    if (preparedPersonel.id !== opening.personnelId) {
      throw new Error('Personel ile vergi opening aynı personel kimliğini taşımalıdır.');
    }
    if (isNative) {
      await tauriBridge.savePersonnelAndTaxOpening(
        toPayrollBoundaryDto(preparedPersonel) as unknown as Personel,
        toPayrollBoundaryDto(opening) as unknown as PersonelTaxOpening
      );
      await loadData();
      return;
    }

    if (!authoritativePayload) {
      throw new Error('Personel kaydedilemedi: yetkili veri snapshotı bulunamadı.');
    }

    if (commitAuthoritativePayload) {
      await commitAuthoritativePayload(async (current) => {
        assertUniqueTcNo(current.personeller, preparedPersonel);
        mergeBrowserTaxOpening(current.taxOpenings, opening);

        const existingPersonel = current.personeller.find(
          (person) => person.id === preparedPersonel.id
        );
        const personHasFinancialChange =
          !existingPersonel || personnelHasFinancialChanges(existingPersonel, preparedPersonel as Personel);
        const existingOpening = current.taxOpenings?.find(
          (item) => item.personnelId === opening.personnelId && item.year === opening.year
        );
        const openingHasFinancialChange = taxOpeningHasFinancialChanges(
          existingOpening,
          opening as PersonelTaxOpening
        );
        const mutations: PayrollMutation[] = [];
        if (personHasFinancialChange) {
          mutations.push({ kind: 'PERSON', personnelId: preparedPersonel.id });
        }
        if (openingHasFinancialChange) {
          mutations.push({
            kind: 'PERSON_TAX_YEAR',
            personnelId: opening.personnelId,
            taxYear: opening.year,
          });
        }
        const impact = mutations.length
          ? await evaluateBrowserMutations(mutations, payrollDatasetFromPayload(current))
          : undefined;
        const existingInCurrent = current.personeller.find(
          (person) => person.id === preparedPersonel.id
        );
        const exactPersonel = mergePayrollUiIntoBoundary(existingInCurrent, preparedPersonel);
        const personeller = existingInCurrent
          ? current.personeller.map((person) =>
              person.id === preparedPersonel.id ? exactPersonel : person
            )
          : [...current.personeller, exactPersonel];
        return {
          ...current,
          personeller,
          taxOpenings: mergeBrowserTaxOpening(current.taxOpenings, opening),
          bordrolar: impact ? applyBrowserPayrollImpact(current.bordrolar, impact) : current.bordrolar,
          retroBatches: impact
            ? applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact)
            : current.retroBatches,
        };
      });
      return;
    }

    assertUniqueTcNo(authoritativePayload.personeller, preparedPersonel);
    mergeBrowserTaxOpening(authoritativePayload.taxOpenings, opening);
    const existingPersonel = authoritativePayload.personeller.find((person) => person.id === preparedPersonel.id);
    const personHasFinancialChange =
      !existingPersonel || personnelHasFinancialChanges(existingPersonel, preparedPersonel as Personel);

    const existingOpening = authoritativePayload.taxOpenings?.find(
      (o) => o.personnelId === opening.personnelId && o.year === opening.year
    );
    const openingHasFinancialChange = taxOpeningHasFinancialChanges(
      existingOpening,
      opening as PersonelTaxOpening
    );

    const mutations: PayrollMutation[] = [];
    if (personHasFinancialChange) {
      mutations.push({ kind: 'PERSON', personnelId: newPersonel.id });
    }
    if (openingHasFinancialChange) {
      mutations.push({
        kind: 'PERSON_TAX_YEAR',
        personnelId: opening.personnelId,
        taxYear: opening.year,
      });
    }

    const impact = mutations.length > 0 ? await evaluateBrowserMutations(mutations) : undefined;
    updateAuthoritativePayload((current) => {
      assertUniqueTcNo(current.personeller, preparedPersonel);
      mergeBrowserTaxOpening(current.taxOpenings, opening);
      const existingInCurrent = current.personeller.find((person) => person.id === preparedPersonel.id);
      const exactPersonel = mergePayrollUiIntoBoundary(existingInCurrent, preparedPersonel);
      const personeller = existingInCurrent
        ? current.personeller.map((person) =>
            person.id === preparedPersonel.id ? exactPersonel : person
          )
        : [...current.personeller, exactPersonel];
      return {
        ...current,
        personeller,
        taxOpenings: mergeBrowserTaxOpening(current.taxOpenings, opening),
        bordrolar: impact ? applyBrowserPayrollImpact(current.bordrolar, impact) : current.bordrolar,
        retroBatches: impact ? applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact) : current.retroBatches,
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

  const handleDeleteDonem = async (periodId: string) => {
    if (isNative) {
      await tauriBridge.deletePeriod(periodId);
      await loadData();
      return;
    }
    const period = latestPayrollDataset.current.periods.find((item) => item.id === periodId);
    if (!period) throw new Error('Silinecek dönem bulunamadı.');
    // The shared mutation policy blocks deletion of finalized history and any
    // deletion that would invalidate downstream finalized payroll or retro data.
    const impact = await payrollEngine.evaluateMutationPolicy({
      kind: 'PERIOD_FROM_POSITION',
      startDate: period.baslangicTarihi,
      taxYear: period.taxYear,
      taxMonth: period.taxMonth,
    }, latestPayrollDataset.current);
    assertBrowserMutationImpactAllowed(impact);
    updateAuthoritativePayload((current) => deletePeriodFromPayload(current, periodId, impact));
  };

  const handleCreateDonem = async (
    newDonem: BordroDonemi,
    kurumDegerleri?: PayrollPeriodInstitutionSettings
  ) => {
    if (isNative) {
      if (kurumDegerleri) await tauriBridge.savePeriodWithSettings(newDonem, kurumDegerleri);
      else await tauriBridge.savePeriod(newDonem);
      await loadData();
      return;
    }
    const existing = donemler.find((period) => period.id === newDonem.id);
    // A brand-new period is empty; it can only affect later periods of the
    // same tax year (PERIOD_INSERT). Editing an existing period keeps the
    // broader positional policy.
    const positionMutations: PayrollMutation[] = [{
      kind: existing ? 'PERIOD_FROM_POSITION' : 'PERIOD_INSERT',
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
      const preservedSettings = kurumDegerleri
        ? (() => {
            const exactSettings = mergePayrollUiIntoBoundary(previousSettings, kurumDegerleri);
            return toPayrollBoundaryDto(reconcileStatutorySnapshot(previousSettings, exactSettings));
          })()
        : undefined;
      return {
        ...current,
        donemler,
        kurumDegerleriMap: preservedSettings
          ? { ...current.kurumDegerleriMap, [newDonem.id]: preservedSettings }
          : current.kurumDegerleriMap,
        annualPayrollParameters: ensureBoundaryAnnualPayrollParameters(
          current.annualPayrollParameters,
          newDonem.taxYear
        ),
        bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact),
        retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
      };
    });
  };

  const handleSaveKurumDegerleri = async (settings: PayrollPeriodInstitutionSettings) => {
    const period = donemler.find((period) => period.id === settings.donemId);
    if (!period) throw new Error('Dönem kurum ayarları için bordro dönemi bulunamadı.');
    if (isNative) {
      await tauriBridge.saveInstitutionSettings(settings);
      await loadData();
      return;
    }
    const persistedSettings = authoritativePayload?.kurumDegerleriMap?.[settings.donemId];
    if (persistedSettings) {
      const unchanged = sameSettingsValue(
        toPayrollBoundaryDto(reconcileStatutorySnapshot(
          persistedSettings,
          mergePayrollUiIntoBoundary(persistedSettings, settings)
        )),
        persistedSettings
      );
      // A no-op save must not invalidate the period and every later period.
      if (unchanged) return;
    }
    const impact = await evaluateBrowserMutations({ kind: 'PERIOD', periodId: settings.donemId });
    updateAuthoritativePayload((current) => ({
      ...current,
      kurumDegerleriMap: {
        ...current.kurumDegerleriMap,
        [settings.donemId]: (() => {
          const previousSettings = current.kurumDegerleriMap[settings.donemId];
          const exactSettings = mergePayrollUiIntoBoundary(previousSettings, settings);
          return toPayrollBoundaryDto(reconcileStatutorySnapshot(previousSettings, exactSettings));
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
    validatePuantajForSave(updatedPuantaj);
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

  const handleSavePuantajBatch = async (
    updatedAttendances: PersonelPuantaj[],
    bulkIntent?: PuantajBulkCommitIntent
  ) => {
    if (updatedAttendances.length === 0) return;

    const seen = new Set<string>();
    for (const attendance of updatedAttendances) {
      validatePuantajForSave(attendance);
      const key = `${attendance.personelId}\u0000${attendance.donemId}`;
      if (seen.has(key)) {
        throw new Error('Toplu puantaj kaydında aynı personel ve dönem birden fazla kez bulunuyor.');
      }
      seen.add(key);
    }

    if (isNative) {
      await tauriBridge.saveAttendances(updatedAttendances);
      await loadData();
      return;
    }

    const applyBatch = (
      current: PayrollStorageDto,
      impact: MutationImpact,
      batchAttendances: PersonelPuantaj[]
    ): PayrollStorageDto => {
      const attendancesById = new Map(
        current.puantajlar.map((attendance) => [attendance.id, attendance] as const)
      );
      for (const attendance of batchAttendances) {
        const exactAttendance = mergePayrollUiIntoBoundary(
          attendancesById.get(attendance.id),
          attendance
        );
        attendancesById.set(attendance.id, exactAttendance);
      }
      return {
        ...current,
        puantajlar: [...attendancesById.values()],
        bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact),
        retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
      };
    };
    if (commitAuthoritativePayload) {
      await commitAuthoritativePayload(async (current) => {
        let batchAttendances = updatedAttendances;
        if (bulkIntent) {
          const period = current.donemler.find((item) => item.id === bulkIntent.periodId);
          if (!period) throw new Error('Toplu puantajın hizmet dönemi güncel snapshotta bulunamadı.');
          const targetPersonnel = current.personeller.filter((person) =>
            bulkIntent.personnelIds.includes(person.id)
          );
          const latestPlan = createPuantajBulkPlan({
            personnel: targetPersonnel,
            attendances: current.puantajlar,
            sickLeaveRecords: current.sickLeaveRecords,
            periodId: period.id,
            periodStartInclusive: period.baslangicTarihi,
            periodEndInclusive: period.bitisTarihi,
            dates: bulkIntent.dates,
            code: bulkIntent.code,
          });
          if (latestPlan.reportMismatches.length > 0) {
            throw new Error(
              `Toplu kayıt durduruldu: ${latestPlan.reportMismatches.length} hastalık raporu günü R koduyla eşleşmiyor. Önce rapor ve puantaj kaydını düzeltin.`
            );
          }
          batchAttendances = latestPlan.updates;
        }
        const mutations = batchAttendances.map((attendance) => ({
          kind: 'PERSON_PERIOD' as const,
          personnelId: attendance.personelId,
          periodId: attendance.donemId,
        }));
        const impact = await evaluateBrowserMutations(mutations, payrollDatasetFromPayload(current));
        return applyBatch(current, impact, batchAttendances);
      });
    } else {
      const mutations = updatedAttendances.map((attendance) => ({
        kind: 'PERSON_PERIOD' as const,
        personnelId: attendance.personelId,
        periodId: attendance.donemId,
      }));
      const impact = await evaluateBrowserMutations(mutations);
      updateAuthoritativePayload((current) => applyBatch(current, impact, updatedAttendances));
    }
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
    updateAuthoritativePayload((current) => ({
      ...current,
      taxOpenings: mergeBrowserTaxOpening(current.taxOpenings, opening),
      bordrolar: applyBrowserPayrollImpact(current.bordrolar, impact),
      retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
    }));
  };

  const handleSaveSickLeaveRecord = async (record: SickLeaveRecord) => {
    if (isNative) {
      // The native repository owns sick leave + puantaj persistence in one
      // transaction, including overlap validation. Do not pre-write attendance.
      await tauriBridge.saveSickLeaveRecord(record);
      await loadData();
      return;
    }
    validateSickLeaveRecord(record);
    for (const other of sickLeaveRecords) {
      if (other.id !== record.id && other.personnelId === record.personnelId) {
        if (record.startDate <= other.endDate && record.endDate >= other.startDate) {
          throw new Error(
            `Rapor tarihleri çakışıyor: ${record.startDate}–${record.endDate} aralığı, ${other.id} kaydındaki ${other.startDate}–${other.endDate} aralığıyla örtüşüyor. Örtüşen raporlar ayrı episode olarak kaydedilemez.`
          );
        }
      }
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

      const nextPuantajlar = syncPuantajForSickLeaveSave({
        record,
        existingRecord: existing,
        donemler: current.donemler,
        puantajlar: current.puantajlar,
      });

      return {
        ...current,
        sickLeaveRecords: nextRecords,
        puantajlar: nextPuantajlar,
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
        retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
      };
    });
  };

  const handleDeleteSickLeaveRecord = async (id: string) => {
    const record = sickLeaveRecords.find((item) => item.id === id);
    if (isNative) {
      await tauriBridge.deleteSickLeaveRecord(id);
      await loadData();
      return;
    }
    const impact = record
      ? await evaluateBrowserMutations({
          kind: 'PERSON_FROM_DATE',
          personnelId: record.personnelId,
          effectiveFrom: record.startDate,
        })
      : null;
    updateAuthoritativePayload((current) => {
      const nextPuantajlar = record
        ? syncPuantajForSickLeaveDelete({
            deletedRecord: record,
            donemler: current.donemler,
            puantajlar: current.puantajlar,
          })
        : current.puantajlar;

      return {
        ...current,
        sickLeaveRecords: current.sickLeaveRecords.filter((item) => item.id !== id),
        puantajlar: nextPuantajlar,
        bordrolar: record
          ? applyBrowserPayrollImpact(current.bordrolar, impact!)
          : current.bordrolar,
        retroBatches: record
          ? applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact!)
          : current.retroBatches,
      };
    });
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
        .filter((item) => (item.accrualId || item.id) !== eventId && item.id !== event.id && (!event.accrualId || item.accrualId !== event.accrualId)),
      retroBatches: applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
    }));
  };

  const handleSaveBordro = async (
    updatedBordro: PayrollBoundaryPayroll,
    calculationSnapshot?: PayrollDatasetSnapshot,
    saveSource?: 'ENGINE_FINALIZATION'
  ) => {
    if (isNative) {
      // Native persistence already invalidates downstream calculated rows in
      // one transaction; re-fetch the whole ledger for the UI.
      await loadData();
      return;
    }
    const policySnapshot = calculationSnapshot ?? latestPayrollDataset.current;
    if (calculationSnapshot) {
      assertPayrollCalculationSnapshotCurrent(
        calculationSnapshot,
        latestPayrollDataset.current
      );
    }
    const existing = policySnapshot.payrolls.find(
      (payroll) => payroll.id === updatedBordro.id || payroll.accrualId === updatedBordro.accrualId
    );
    if (saveSource === 'ENGINE_FINALIZATION') {
      if (!existing || existing.status !== 'CALCULATED' || updatedBordro.status !== 'FINALIZED') {
        throw new Error('Yalnız hesaplanmış bordro, yetkili finalize_payroll sonucu ile kesinleştirilebilir.');
      }
    } else if (!calculationSnapshot && existing) {
      validatePayrollStatusTransition(existing.status, updatedBordro.status);
    }
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
    const impact = await evaluateBrowserMutations(mutation, policySnapshot);
    if (calculationSnapshot) {
      assertPayrollCalculationSnapshotCurrent(
        calculationSnapshot,
        latestPayrollDataset.current
      );
    }
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
          ? applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact).map((batch) =>
              batch.id === updatedBordro.accrualId
                ? {
                    ...batch,
                    status: 'FINALIZED' as const,
                    settlementStatus: 'PAID' as const,
                    finalizedAt: updatedBordro.sonGuncellemeTarihi,
                  }
                : batch
            )
          : applyBrowserRetroBatchImpact(current.retroBatches ?? [], impact),
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
    const zeroDifference = batch.totalGrossDelta === 0 &&
      batch.payableSettlementAmount === 0 &&
      (batch.offsetSettlementAmount ?? 0) === 0 &&
      (batch.settlementStatus ?? 'UNSETTLED') === 'UNSETTLED';
    if (
      batch.payableSettlementAmount !== 0 ||
      (!zeroDifference && !['OVERPAYMENT', 'SETTLED_BY_OFFSET'].includes(batch.settlementStatus ?? ''))
    ) {
      throw new Error('Payment event olmadan yalnız açık fazla tahakkuk veya mahsupla kapanan settlement batch’i saklanabilir.');
    }
    // Mirrors the native settlement-only save: a settlement ledger has no
    // payment event, so a linked older event is retired atomically with the
    // ledger rewrite. FINALIZED is never touched; a live DRAFT/CALCULATED
    // event is retired only when the replay resolves to no difference.
    const linkedPayments = payrollDataset.payrolls.filter((payroll) => payroll.accrualId === batch.id);
    const blockingPayment = linkedPayments.find((payroll) =>
      payroll.status === 'FINALIZED' || (payroll.status !== 'STALE' && !zeroDifference)
    );
    if (blockingPayment) {
      throw new Error(`${batch.id} retro payment event'i ${blockingPayment.status} durumunda; event silinmeden fazla tahakkuk batch'i saklanamaz.`);
    }
    if (isNative) {
      await tauriBridge.saveRetroAdjustmentBatch(batch, canonicalResult.allocations);
      await loadData();
      return;
    }
    if (!authoritativePayload) throw new Error('Yetkili veri snapshot’ı hazır değil.');
    const retireImpact = linkedPayments.length
      ? await evaluateBrowserMutations(linkedPayments.map((payroll) => ({
          kind: 'ACCRUAL_DELETE' as const,
          personnelId: payroll.personelId,
          periodId: payroll.donemId,
          accrualId: batch.id,
        })))
      : null;
    const isRetiredPayment = (payroll: { accrualId?: string | null }) =>
      linkedPayments.length > 0 && payroll.accrualId === batch.id;
    const exactBatch = toPayrollBoundaryDto(batch) as unknown as NonNullable<PayrollStorageDto['retroBatches']>[number];
    const exactAllocations = toPayrollBoundaryDto(canonicalResult.allocations) as unknown as NonNullable<PayrollStorageDto['retroAllocations']>;
    const datasetWithBatch: PayrollDatasetSnapshot = {
      ...payrollDataset,
      payrolls: payrollDataset.payrolls.filter((payroll) => !isRetiredPayment(payroll)),
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
    updateAuthoritativePayload((current) => {
      const retiredBordrolar = retireImpact
        ? applyBrowserPayrollImpact(current.bordrolar, retireImpact).filter((item) => !isRetiredPayment(item))
        : current.bordrolar;
      const retiredBatches = retireImpact
        ? applyBrowserRetroBatchImpact(current.retroBatches ?? [], retireImpact)
        : current.retroBatches ?? [];
      return {
        ...current,
        compensationRevisions: current.compensationRevisions ?? [],
        compensationRevisionOverrides: current.compensationRevisionOverrides ?? [],
        retroBatches: [...retiredBatches.filter((item) => item.id !== batch.id), exactBatch],
        retroAllocations: [
          ...(current.retroAllocations ?? []).filter((item) => item.batchId !== batch.id),
          ...exactAllocations,
        ],
        bordrolar: applyBrowserPayrollImpact(retiredBordrolar, impact),
      };
    });
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
    handleSavePersonelAndTaxOpening,
    handleDeletePersonel,
    handleCreateDonem,
    handleDeleteDonem,
    handleSaveKurumDegerleri,
    handleSavePuantaj,
    handleSavePuantajBatch,
    handleSaveTaxOpening,
    handleSaveSickLeaveRecord,
    handleSaveAnnualPayrollParameters,
    handleDeleteSickLeaveRecord,
    handleDeleteBordro,
    handleSaveBordro,
    handleSaveCompensationRevision,
    handleCalculateRetroPreview,
    handleSaveRetroBatch,
    handleCreateRetroPayment,
  };
}
