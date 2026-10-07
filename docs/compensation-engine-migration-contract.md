# Compensation Engine Geçiş Sözleşmesi

Bu belge mevcut koddan doğrulanan davranışları ve henüz uygulanmamış hedef mimari kararlarını ayırır. Görev sırası ve sahiplikler [görev haritasında](compensation-engine-migration-tasks.md).

## Güncel uygulama durumu — 2026-10-07

1–8B kod görevleri kendi kapsamlarıyla kabul edildi. Aşağıdaki ilk keşif tabloları ve aşama notları geçiş öncesi davranışı da kaydeder; güncel uygulamanın kanıtı görev haritasındaki son kabul kayıtları ve 8A/8B handoff'larıdır. Ordinary NORMAL/native/WASM/browser hesap ve preflight full-policy timeline kullanır; aktif period DTO'su yalnız yasal/kesinti/tax-application ve ayrı tahakkuk girdilerini taşır. Schema2 provenance ve canonical finansal replay zorunluluğu yeni bordrolarda uygulanır.

V5/V6 arşiv uyumluluğu için açık `LegacyBackupPayrollInput` kullanan ayrı doğrulama adapter'ı eski hesabı exact kayıtlı kaynaklarla karşılaştırır. Bu adapter yeni policy üretmez ve normal/retro hesap fallback'i değildir. Eski raise/transport kaynaklarının fiziksel olarak tamamen silindiği veya tüm release artefaktlarından çıkarıldığı iddia edilmez; ordinary hesap grafiği bu kaynakları kullanmaz. Snapshot'sız historical retro exact source olmadan açık hata verir.

Worker'ın son workspace, native, WASM ve browser kabul testleri başarılıdır; genel TypeScript/lint matcher ve PuantajGrid tanılamaları ayrı mevcut borç olarak açıktır. Canlı kullanıcı verisine başlangıç politikası kaydedilmedi, kullanıcı adına onay verilmedi ve deployment/operasyonel activation yapılmadı. Aşağıdaki operational checklist'in kullanıcıya ait baseline/veri adımları bu kod kabulünden ayrı kalır.

## Karar ilkeleri

- NORMAL ücret kaynağı kurum kapsamındaki immutable, tam durumlu CompensationPolicyVersion timeline’ıdır.
- CompensationRevision geçmiş hakkı, CompensationPolicyVersion ileri tarihli normal ücreti temsil eder; birbirinin yerine geçmezler.
- Legacy calculate_payroll parity oracle olarak kalır; parity, snapshot, restore, loader, invalidation ve kullanıcı onaylı baseline kapıları geçmeden production yolu değişmez.

## Veri sahipliği

| Veri | Otorite / zaman | Kodla doğrulanan bugünkü sınır |
|---|---|---|
| Ücret ve yan hak | Kurum kapsamlı policy timeline; effectiveFrom çalışma tarihidir. | Bugün DonemselKurumDegerleri ve geçmiş döneme bakan raise_transition kullanılır [raise_transition.rs](../crates/payroll-core/src/raise_transition.rs#L41). |
| Yasal girdiler | Ayrı yıllık ve dönemsel statutory ayar/snapshot. | Mevcut modelde karışık, ancak StatutoryParameterSnapshot ve segment türleri zaten ayrıdır [models.rs](../crates/payroll-core/src/models.rs#L866). Compensation’a taşınmaz. |
| Vergi | PersonelTaxOpening + authoritative bordro/payment-event zinciri. | GV/asgari GV opening kendi period ID’sine bağlı [models.rs](../crates/payroll-core/src/models.rs#L84). Vergi yılı/ayı, paymentDate ve çalışma dönemi ayrıdır. |
| Puantaj/rapor | Günlük tarihli attendance ve sick-leave dates. | Bordro mevcutta puantaj özetini saklar; gün map’i ve paid report dates yoktur [models.rs](../crates/payroll-core/src/models.rs#L647). Hedefte snapshot’a embed edilir. |
| Bordro provenance | Used input snapshots + financial result + calculation version. | Statutory, GV, PEK ve iş primi snapshot’larının bir kısmı vardır; compensation snapshot yoktur [models.rs](../crates/payroll-core/src/models.rs#L677). |
| Retro | Revision, override, allocation, settlement/payment-event graph. | V5 graph ve FINALIZED payment bağlantıları korunur [models.rs](../crates/payroll-core/src/models.rs#L379). |
| Sendika/BES | Açık InstitutionPayrollDeductionSettings ve personel seçimleri; bordro snapshot’ında ikisi de. | Statutory değildir, compensation’a taşınmaz. Mevcut kaynak precedence’i [calculations.rs](../crates/payroll-core/src/calculations.rs#L1464). |
| Yemek vergi uygulaması | PayrollTaxApplicationPolicy ve bordro snapshot’ı. | yemekTamGvIstisnaDamgaTabi compensation/statutory değildir; günlük yasal cap’i değiştirmez [payroll_engine.rs](../crates/payroll-core/src/payroll_engine.rs#L3417). |

İlk kapsam kurum genelinde tek timeline’dır: bugünkü dönem ayarları kurumsal, sadece iş primi oranı personel grubuna göre seçilir. Personel başına policy scope’u eklenmez. Grup, hizmet yılı ve kişi kesinti tercihleri geçmiş bordro snapshot’ında saklanır.

## Full-state alanları ve tarih

Her sürüm tam olarak şu ücret durumunu taşır: gunlukTabanUcret, gunlukYemek, birlestirilmisSosyalYardim, gunlukVasitaYol, giyimYardimi, hizmetZammiBirimi, isPrimiYuzde/isPrimiGruplari, geceCalismaPrimiYuzde, geceCalismaTatiliPrimiYuzde, ekOdeme ve digerGelirVarsayilan.

- State delta yoktur. Eksik finansal alan resolver’da defaultlanmaz. Daily/money alanları için 0 açık sıfırdır; taban ücret pozitif, tutarlar negatif olmayan değer olmalıdır.
- Gece oranları full-state’te eksiksiz Decimal’dir; 0 kapalı, null/eksik invalid’dir.
- isPrimiGruplari tam listedir; personel grubu için tek aktif eşleşme gerekir. Oran yüzde birimidir (65 = %65; bugünkü 0–100 validation korunur). Bugünkü motor isPrimiYuzde’den fallback yapmaz; aktif grup oranı otoritedir [calculations.rs](../crates/payroll-core/src/calculations.rs#L594). Eski global alan import/rapor için taşınsa bile motor kullanmaz.
- ekOdeme ve digerGelirVarsayilan optional olabilir: null absent kalem, decimal string "0" açık sıfır kalemidir. Alanın kendisi eksik olamaz.
- Arithmetic Rust Decimal’dır, JSON para alanları decimal-string’dir; TS sınırında number reddedilir [decimalBoundary.ts](../src/services/payrollEngine/decimalBoundary.ts#L100). UI artış helper’ı exact decimal hesaplayıp yeni full-state yazar, yüzde kuralı saklamaz/replay etmez.
- effectiveFrom geçerli ISO çalışma tarihidir, günü dahil. paymentDate/taxYear/taxMonth/periodId ile karıştırılamaz. 15–14 dönem iki uç dahil; dönem yıl/ayı start ayıdır [payroll_engine.rs](../crates/payroll-core/src/payroll_engine.rs#L151), [models.rs](../crates/payroll-core/src/models.rs#L67).
- Resolver input dönemi [start,end] inclusive, timeline aralığı [effectiveFrom,nextEffectiveFrom) kabul edilir; persisted segment endpoints inclusive olur. Start-day ve end-day değişikliği dahildir.
- resolve_compensation_segments(start,end,versions), başlangıcı kapsayan active full-state’i ister; tarih sıralı, boşluksuz segment verir. Uzun ara kendi başına timeline gap’i değildir. Duplicate date/active version, invalid state/date, reverse order, missing baseline, dangling/farklı scope supersedes, cycle/fork hata olur.
- İlk baseline ve farklı tarihteki olağan sürüm supersedes istemez. Aynı tarih düzeltmesi eski aktif leaf’i supersede eden yeni immutable tam kayıttır; zincir sonunda tek aktif leaf kalır.
- Timeline payroll period FK’si almaz; dönem silme veya kurum ayarları cascade’i policy history’sini silemez.

CompensationRevision geriye dönük hak, CompensationPolicyVersion ileri normal politika sahibi olur. Geçmiş + gelecek etkili kararda iki kayıt gerekir; policy eski NORMAL hakkı değiştirmez, revision resolver baseline’ı olmaz.

## Resolver, atomik tarihler ve parity blocker

Compensation ve statutory modeller ayrı kalır. Dönem start/end, compensation effective dates ve statutory effective dates yemek/SGK/GV için atomik tarih dilimlerini oluşturur. Atomik tarih aralığı ile gelir yuvarlama aralığı aynı kavram değildir. Eski motorun tam compensation/raise yuvarlama sınırı günlük oran değişmese de korunur; yalnız rate eşitliğine bakarak dilim birleştirmek 310.15/310.16 yemek farkı yaratmıştır. Statutory-only veya gerçek yol-only tarih diğer günlük gelirler için yeni yuvarlama sınırı olmaz. Her effective-date'in bu rollerden hangisini taşıdığı mevcut full-state modelinde açık değildir; B-RND-02 kapısı aşağıda bu eksikliği izler. Yemek uygulama tercihi ve günlük yasal cap’ler aynı tarihte geçerli ayrı input’lardır.

### B-RND-01 — legacy yemek istisnası yuvarlaması korunur; doğrulama Görev 4'te

Bugünkü calculate_normal_meal_exemptions statutory segmentleri dolaşıp her parça için calculate_meal_income çağırır [payroll_engine.rs](../crates/payroll-core/src/payroll_engine.rs#L727), [calculations.rs](../crates/payroll-core/src/calculations.rs#L32). round2 midpoint-nearest-even’dir [calculations.rs](../crates/payroll-core/src/calculations.rs#L53). Günlük yemek 10.005, iki çalışılan gün: tek economic dilim round2(20.010)=20.01; statutory-only iki tek günlük dilim round2(10.005)+round2(10.005)=20.00. Günlük cap’ler yeterliyse exemption/vergilendirilebilir yemek 0.01 değişir.

Kullanıcının mevcut finansal sonuçları değiştirmeme talimatı gereği legacy statutory-boundary exemption rounding'i korunur. Yemek gelir toplamının hesap dilimleri ile yemek istisnasının yasal kapasite dilimleri farklıdır: statutory sınırı istisna hesabının ilgili girdisidir; unrelated gelirleri bölmeme kuralı bu sınırı istisna hesabından kaldırmaz. Yeni cent allocation/rounding formülü bu migration'a eklenmez. Görev 4 örneği tam bordro fixture'ında ve doğrudan helper seviyesinde doğrular; gelir toplamı, istisna ve vergi sonucu ayrı karşılaştırılır. N-segment genişlemede tam compensation yuvarlama sınırı ile yol-only boundary ayrımı B-RND-02'nin açık typed semantiğine tabi olacaktır. Bu kararın uygulanabilirliğini çürüten gerçek parity farkı bulunursa blocker yeniden açılır; test daraltılarak veya expected değiştirilerek kapatılmaz.

### B-RND-02 — tek ücret timeline'ında yol ve kazanç yuvarlama sınırı; proof kabul, kalıcı entegrasyon bekliyor

Görev 4 fractional parity kanıtında road `None` compensation dilimleri başına `220.05`; `Some([])` tek dönem toplamında `220.06`; tarihli road-only değişimde `300.06` üretir. Road-only tarih base/meal/premium/night dilimini bölmez. Günlük tutarlar tek full-state timeline'da ifade edilebilir, ancak version effective-date tek başına hem boundary kapsamını hem yolun toplama/yuvarlama semantiğini tanımlamaya yetmez. Mevcut test-only `DatedRoadRateSchedule` ek monetary input ile parity sağlayabilir; hedef mimari için bu yeterli değildir.

Orchestrator yönü: ikinci authoritative yol tutarı timeline'ı veya yeniden adlandırılmış `transportRateChanges` açılmaz. Bütün yol tutarları `CompensationPolicyVersion.state.gunlukVasitaYol` içindedir; yol-only değişiklik de tam state version üretir. Görev 4'ün test-only candidate proof'u kaynak incelemesi ve bağımsız 20 shadow + 38 payroll + 14 raise regression testiyle kabul edildi. Aşağıdaki explicit metadata kalıcı sözleşme olarak seçildi; bu alanlar yüzde/delta replay kuralı değildir.

- Immutable `CompensationPolicyVersion.boundaryScope`: `FULL_EARNING` veya `ROAD_ONLY`.
- Immutable `CompensationPolicyVersion.roadRoundingMode`: `PER_COMPENSATION_SLICE` veya `PERIOD_TOTAL`.
- Alanlar yeni version'da zorunludur; serde/schema/SQL financial default veya source/reason/rate-equality inference yoktur. Düzeltme yeni version ve açık flag'lerle yapılır.
- `FULL_EARNING` ilgili günlük kazançlar için oranlar eşit olsa da yeni yuvarlama dilimi açar. Dönem içi `ROAD_ONLY`, önceki aktif full state ile yol dışındaki bütün state alanları eşitse kabul edilir; diğer gelirlerin yuvarlama dilimini bölmez. Dönem başlangıcındaki son full state, scope ne olursa olsun doğal baseline'dır; eski full-boundary tarihini bulmak için compensation geçmişi yüklenmez.
- Tek bordro hizmet aralığında resolved active version'ların road mode'u aynı olmalıdır; dönem içindeki karma mode fail-closed olur. Metadata sırf değerler eşit diye birleştirilmez veya tahmin edilmez.
- `PER_COMPENSATION_SLICE`: her full earning dilimindeki C+GC günlerinin o tarihlerdeki full-state yol tutarlarını topla, ayrı round2 uygula, sonra dilimleri topla. `PERIOD_TOTAL`: bütün dönem C+GC günlerinin o tarihlerdeki full-state yol tutarlarını topla, sonunda tek round2 uygula.
- Resolved compensation snapshot her segmentte iki flag'i de taşır. Yeni snapshot schema 2 olur; flag'siz schema 1 finansal anlamı sessizce yeniden üretilmez. Eski/missing metadata legacy bordroları ve V1–V5 authority kuralları korunur.

Görev 4B tek owner olarak domain/resolver, SQLite/repository, snapshot schema/validator, native/WASM/browser boundary ve V6 backup kalıcılığını bu sözleşmeye geçirir. Ardından shadow sidecar kaldırılarak kesin parity aynı persisted final model üzerinden yeniden çalıştırılır. Experimental flag'siz policy/snapshot için açık unsupported/quarantine hata yolu gerekir; otomatik financial backfill veya veri silme yoktur. Loader/mutation/UI activation Görev 5/7'deki sahipliğini korur. 4B teslimi ve orchestrator package consolidation tamamlanmadan 5A veya normal production switch açılmaz.

## Finansal formül ve legacy davranış tablosu

round2 bugünkü Decimal::round_dp(2), midpoint-nearest-even’dir. GV ve SGK/işsizlik primleri için ayrı midpoint-away-from-zero politikaları vardır [calculations.rs](../crates/payroll-core/src/calculations.rs#L53). Multi-segment kuralları legacy parity yanında golden ile de sabitlenir.

| Kalem | Kodla doğrulanan kaynak/formül | Hak/ağırlık | Rounding/segment |
|---|---|---|---|
| Taban ücret | daily base × ücret hak kodları C,T,G,İ,GC,GÇT; paid sick wage ayrıca eklenir | Attendance hak günü + doğrulanmış paid report dates | Bugünkü pre/post tam compensation/raise dilimleri round2, sonra toplanır; oran eşitliği bu sınırı kaldırmaz. Yol-only kapsam B-RND-02'ye tabidir. |
| Ücretli rapor | Her yıl sıralı ilk 5 epizodun ilk iki tarihi; period içinde R ile eşleşirse daily base ekle [payroll_engine.rs](../crates/payroll-core/src/payroll_engine.rs#L597) | Exact sick dates, epizot başına en çok 2 gün | Günlük tutar × paid days; round2. Tarihler snapshot’lanır. |
| Yemek | daily meal × (C+GC) [calculations.rs](../crates/payroll-core/src/calculations.rs#L989) | Yalnız C+GC, calendar weighting yok | Tam compensation/raise dilimi başına round2; aynı oranlı dilimler kör birleştirilmez. Exemption compensation × statutory kesişimidir; B-RND-01 korunur. |
| Yol | daily road × C+GC; legacy dated transport varsa günlük effective rate [transport_rates.rs](../crates/payroll-core/src/transport_rates.rs#L31) | Yalnız C+GC | Legacy None compensation dilimi başına; Some([])/dated override dönem toplamında round2. Final tek-timeline temsili B-RND-02'de BLOCKED. Road-only tarih diğer günlük gelirleri bölmez. |
| İş primi | daily base × (aktif grubun oranı/100) × (C+GC) [calculations.rs](../crates/payroll-core/src/calculations.rs#L964) | Hak günü C+GC | Tam compensation/raise dilimi başına tutar round2, ardından legacy detail merge; yüzde katsayı değil yüzde birimidir. |
| Gece / gece tatili | daily base × ilgili oran/100 × GC veya GÇT [calculations.rs](../crates/payroll-core/src/calculations.rs#L475) | GC ve GÇT ayrı | İki ayrı gelir; tam compensation/raise dilimi başına round2. |
| Sosyal yardım | Period fixed amount; raise’de önceki/sonraki tutar calendar-day weighted | Tüm dönem takvim günleri, attendance yok | (old×days + new×days)/periodDays; tüm paylar toplandıktan sonra tek round2 [raise_transition.rs](../crates/payroll-core/src/raise_transition.rs#L158). |
| Giyim | Sosyal yardımla aynı calendar-weighted period formula | Tüm takvim dönemi | Ağırlıklı toplamdan sonra tek round2; yol/stat boundary break değildir. |
| Hizmet zammı | hizmetYili × hizmetZammiBirimi, calendar-weighted | Tüm takvim günleri | Ağırlıklı toplamdan sonra tek round2; hizmet yılı snapshot input’udur. |
| ekOdeme | NORMAL’de açık default bir kere; SUPPLEMENTAL amount ayrı event. Legacy tediye/TİS listeleri auto accrual değildir [calculations.rs](../crates/payroll-core/src/calculations.rs#L1058). | Period lump-sum | round2; NORMAL value hizmet döneminin son günündeki policy’den tek sefer örneklenir (legacy period setting’iyle uyumlu), pro-rata yok. Ayrı SUPPLEMENTAL tekrar policy’den üretilmez. |
| digerGelirVarsayilan | NORMAL auto-fill açık ise bir kere, yoksa absent | Period lump-sum | round2; period-end policy state’inden örneklenir; null ve açık 0 ayrıdır. |
| Sendika aidatı | Üye değilse 0; pozitif kişi sabiti > pozitif kurum sabiti > yüzde (eksikse legacy %65) × period-end daily base [calculations.rs](../crates/payroll-core/src/calculations.rs#L1464) | Dönem tek kesinti | Percent/100, sonra round2. Kişi/kurum seçimi ve kaynak bordro snapshot’ında. Owner statutory değil deduction policy. |

BES/sendika gibi non-statutory oran/sabit tutar açık InstitutionPayrollDeductionSettings sahibinde; üyelik ve kişisel seçim personeldedir. Bugünkü fixed precedence/fallback parity’de korunur, statutory parametre gibi sınıflandırılmaz.

Tediye/TİS ayrı accrual modelindedir. ManualPayrollIncomeInput yalnız açık kişi/dönem input’unu uygular, yoksa alanı temizler; negatif tutar hata ve round2 uygulanır [payroll_engine.rs](../crates/payroll-core/src/payroll_engine.rs#L852).

Legacy ayrıntıları:

- raise_transition yalnız yemek, sosyal yardım, giyim ve hizmet birimini base wage artışına göre indeksler; artış yüzdesi önce iki ondalıkta round2 olur, cari raw amount değişmişse explicit override sayılır [raise_transition.rs](../crates/payroll-core/src/raise_transition.rs#L95). Diğer income alanları otomatik zamlanmaz.
- Zam ayının ilk günü yeni dilimdedir. zamAylari yoksa önceki period aranmaz; varsa geçmiş dönemler bitişik, ayarları mevcut olmalıdır [raise_transition.rs](../crates/payroll-core/src/raise_transition.rs#L70). Yeni resolver bu bağımlılığı kaldırır.
- Legacy pre-raise grubu önceki ayarda yoksa cari ayarın grubunu eski ayara ekleyebilir [payroll_engine.rs](../crates/payroll-core/src/payroll_engine.rs#L3160).
- transportRateChanges None: dated helper çağrılmaz; zam dilimi varsa yol geliri de eski/yeni settings'in günlük yol tutarlarıyla ayrı hesaplanıp toplanır. Some([]): dated helper önceki split sonucu üzerine yazar ve cari settings'in temel günlük yol tutarını tüm C+GC günlerinde kullanır. Bu nedenle null ile [] zam döneminde farklı ekonomik sonuç verebilir; yalnız provenance farkı değildir [payroll_engine.rs](../crates/payroll-core/src/payroll_engine.rs#L3211), [payroll_engine.rs](../crates/payroll-core/src/payroll_engine.rs#L3247). Some(changes) tarihe göre yol tutarlarını toplar ve sonunda round2 uygular. Legacy fixture/adaptasyon bu ayrımı korur; yeni authority policy timeline'dır.
- Bugünkü DonemselKurumDegerleri compensation dışında yemek tax treatment, statutory, deductions ve tediye/TİS listeleri taşır [models.rs](../crates/payroll-core/src/models.rs#L892); bunlar yeni compensation state’e alınmaz.

## Snapshot, replay ve retro

Yeni NORMAL CALCULATED/FINALIZED bordroda compensation snapshot zorunludur. DRAFT ve sparse legacy restore istisnadır. 3A model sözleşmesi `BordroKaydi.calculationMetadata` alanıdır; eski/missing alan `LEGACY` kabul edilir ve serializer `None` alanını atlar. Yeni motor kökeni `COMPENSATION_POLICY_ENGINE` olarak açıkça yazılır. CALCULATED/FINALIZED NORMAL kayıtta bu köken varsa snapshot zorunluluğu `validate_payroll_calculation_metadata(&BordroKaydi, &BordroDonemi)` ile denetlenir; DRAFT ayrı tutulur.

Snapshot envelope alanları: `snapshotSchemaVersion=1`, semantic `calculationVersion="compensation-engine-v1"`, `compensation.segments` ve `usedInputs`. `ResolvedCompensationSegment` reuse edilir; her segment immutable `versionId`, inclusive `startDate`/`endDate` ve tam `CompensationPolicyState` taşır. Version kimliği tarihsel olarak superseded olsa da finalized snapshot geçerlidir. Build hash semantic hesap sürümü değildir. Bilinmeyen schema/calculation sürümü typed error verir. Paid-sick episode kuralı `first-two-days-first-five-episodes-by-start-year-v1` olarak etiketlenir.

Snapshot’a person group/service year, personal deduction choices, manual/accrual incomes, daily attendance codes, paid report dates, statutory/annual/tax treatment inputs, GV opening anchor ve kullanılmış prior authoritative payroll event/cumulative/PEK/carry state provenance girer. Günlük puantaj ve rapor tarihleri embed edilir; mutable source map veya source hash tek başına yeniden hesaplama kanıtı değildir.

3A snapshot input-owner eşlemesi:

| Otorite | 3A snapshot alanı | Audit/replay kullanımı |
|---|---|---|
| Bordro/period üst kaydı | `BordroKaydi.personelId/donemId/accrualId/accrualType/paymentDate/sequence` + `BordroDonemi` | Kimlik ve çalışma/vergi/ödeme dönemi tekrar snapshot içine kopyalanmaz; public validator bunları parent context ile doğrular. |
| Kurum immutable ücret timeline’ı | `compensation.segments[]` | Tarihli tam ücret state’i; canlı dönem ayarı veya aktif timeline yeniden okunmaz. |
| Kişinin hesapta kullanılan seçimleri | `usedInputs.personnelGroup/serviceYear/personalDeductions` | Grup, hizmet yılı, bireysel kesinti ve GV indirim seçimleri; kimlik dışı kişisel PII eklenmez. |
| Günlük puantaj ve rapor episode’ları | `usedInputs.attendance`, `paidSickLeave.sourceEpisodes/paidDates/ruleVersion` | Tam günlük kodlar ve episode başlangıç-bitişleri ile kullanılan ücretli tarihler; coverage ve R-kodu ilişkisi doğrulanır. |
| Kişi/dönem manuel gelir ve tahakkuk tutarı | `usedInputs.manualIncome/accrualGrossAmount`; tahakkuk kimliği/tarihi/sırası parent kayıtta | Null/zero ve isteğin verilmiş olması korunur; Tediye/TİS listelerinden tahmin yapılmaz. |
| Kurum kesinti ve yemek vergi tercihi | `usedInputs.institutionDeductionSettings` | Kullanılan sendika/BES/DV girdileri ve açık yemek vergi uygulaması; nullable para alanları exact string/null’dır. |
| Dönem ve tarihli yasal girdiler | `usedInputs.statutoryParameters`, ayrıca parent `statutorySnapshot` çıktısı | Günlük taban/tavan/istisna/rate segmentleri ile resolved statutory sonuç birlikte tutulur. |
| Vergi yılı tarifesi | `usedInputs.annualParameters` | Tarifeler ve sigorta GV yıllık tavanı aynı vergi yılına bağlıdır. |
| Açılış ve önceki vergi zinciri | `usedInputs.taxHistory.normalGvOpening/minimumWageGvOpening/previousPaymentEvents/minimumWageMonthlyReferences` | Opening kaynağı/effective tax anchor, kullanılan kümülatifler, yıllık sigorta GV kullanımı ve önceki tahakkuk tüketimleri exact tutarlarla uzlaştırılır. |
| Önceki aynı ödeme ayı PEK/istisna ve carry | `taxHistory.previousPaymentEvents`, `incomingPekCarry` | Önceki event id/date/sequence/type, PEK baz/tavan/tüketim, GV/DV istisnası, outgoing/incoming carry ve source anchor taşınır. |

3A validator schema/bağ/coverage/input tutarlılığını doğrular; finansal replay yapmaz ve canlı settings okumaz. 3A’da legacy `calculate_payroll_with_index` metadata üretmez; `V5` payload, DB, JSON backup, IndexedDB ve runtime loader değişmez. Canonical yeni motor snapshot oluşturma ve exact replay kanıtı daha sonraki engine/persistence/acceptance görevlerindedir.

Auditability ve replayability ayrı kriterdir. Snapshot’ı incelemek audit; aynı exact inputs + desteklenen calculationVersion ile deterministik yeniden hesap replay’dir. FINALIZED geçmiş current Personel/period settings/statutory/attendance/zamAylari ile sessizce yeniden üretilmez. Yeni retro original entitlement persisted input’tan gelir. Legacy replay adapter exact source sağlayamazsa açık LEGACY_REPLAY_UNAVAILABLE; tahmin/default/canlı ayar yok.

Revision precedence, paid authoritative farklar, earning allocations, tax/SGK treatment, worker/employer PEK/carry ve payable/offset/recoverable/outstanding settlement/payment event korunur. Mevcut resolver’da kişi override’ı global override’dan önceliklidir; aynı kapsamda birden fazla override hata verir. STALE revision uygulanmaz; CALCULATED/FINALIZED veya authoritative batch’i olan revision’lar replay’e girer. Revision etkinlik aralığı `[effectiveFrom,effectiveTo]` inclusive’tir. Aynı kişiye uygulanabilir revision’lar `effectiveFrom`, `signedAt`, `updatedAt`, id sırasıyla uygulanır; daha sonra uygulanan değer öncekinin üzerine yazar [retro.rs](../crates/payroll-core/src/retro.rs#L152) [retro.rs](../crates/payroll-core/src/retro.rs#L367). Negative override ve yüzde alanında 100’ü aşan değer reddedilir. Tediye/TİS, revision override’ı değildir; event date ve partial-period geometry ayrı payment event olarak taşınır. V5 restore FINALIZED batch-payment bağlantısını doğrular [migration_service.rs](../src-tauri/src/services/migration_service.rs#L205). Retro yalnız iki gross toplamın farkı değildir.

## Mutation ve FINALIZED koruması

1. Direct ücret etkisi [effectiveFrom,next active version) ile kesişen NORMAL çalışma dönemleridir.
2. Bu bordro köklerinden GV cumulative/yearly limit, same-payment-month GV/DV exemption consumption, PEK, carry, later payment-event ve retro-settlement dependency closure hesaplanır. Gerçek statutory/tax ihtiyaçları ücret dependency’si sayılmaz.
3. Affected DRAFT/CALCULATED stale/recalculate edilir. Tüm closure’daki FINALIZED payroll/batch mutation’ı block eder; direct row CALCULATED olsa bile downstream FINALIZED etkileniyorsa block. FINALIZED kayıt değiştirilmez/stale/recalculate edilmez.
4. Geçmiş NORMAL hak için retro gerekir; güvenli ileri effective date policy eklenebilir.
5. Supersession, affected closure, block check ve stale marking tek transaction’da; başarısızlıkta version ve partial status bırakılmaz.

Mevcut evaluate_payroll_invalidation affected payroll, retro batches ve finalized blockers döndürür [policies.rs](../crates/payroll-core/src/policies.rs#L704); yeni policy date/closure mutation’ı eklenmelidir.

## V6, V5 ve restore contract’ı

Bugünkü JSON backup Rust CURRENT_BACKUP_VERSION=5 ve TS BACKUP_FORMAT_VERSION=5’tir. Native SQLite migration, JSON version, browser IndexedDB DATABASE_VERSION ve generated contract farklı sürüm sınırlarıdır.

| Mevcut garanti | Kanıt | V6’da koruma |
|---|---|---|
| V1/V2 single payroll; V3 multi-accrual/payment event | Native backupVersion<3 seçimi [migration_service.rs](../src-tauri/src/services/migration_service.rs#L1138); web V1/V2/V3 compatibility [payrollPayload.ts](../src/services/storage/payrollPayload.ts#L185) | Eşik 3 olarak kalır; MULTI_ACCRUAL_VERSION sabiti mevcut aramada bulunmadı. |
| V2+ tax opening, sick leave, annual parameters | Native version>=2 kontrolleri | Eşik >=2 olarak kalır; >=6 yapılmaz. |
| V5 retro graph, settlement snapshot ve current replay authority | Current V5 koşulları [migration_service.rs](../src-tauri/src/services/migration_service.rs#L1533), strict V5 schema [payrollPayloadSchema.ts](../src/services/storage/payrollPayloadSchema.ts#L1594) | Semantik eşik 5, V5 ve V6’yı kapsar; CURRENT_VERSION ile karıştırılmaz. |
| Current payload strict ve unsupported future reject | V5 validator exact current version ister | V6 strict schema eklenir; eski payload ayrı adapter’da kalır. |

CURRENT_BACKUP_VERSION 5→6 olurken tüm CURRENT karşılaştırmaları garanti bazında gözden geçirilir. Sadece sabiti artırmak kabul edilmez.

- V6 hem full policy timeline’ı hem payroll compensation snapshots taşır. Native SQLite, browser IndexedDB ve JSON read/write, Decimal, version ilişkileri ve replay proof doğrular. Restore transaction’ı dangling/duplicate supersedes, eksik authoritative snapshot, hatalı financial string veya bozuk ilişki varsa rollback olur.
- V6 current CALCULATED/FINALIZED restore inputs ve canonical replay’i doğrular. Restore bittiğinde DB repository, browser store cache ve açık App yeni dataset instance’ına bağlanır; eski memory state ile calculation yapılamaz.
- V5 ve öncesi okunup restore edilebilir; retro/payment authority korunur. Timeline yoksa yeni normal hesap, explicit user-approved baseline olmadan reddedilir. Eski zamAylari, prior gross veya silinmiş period’dan ücret türetilmez.
- UI aktif period raw rates’i draft prefill olarak gösterebilir. Kullanıcı tarih + tüm full state’i onaylamadan timeline row authoritative değildir. Target ilk baseline 2026-09-15’tir.
- transportRateChanges yeni motorda period input'u olmaz. Legacy import bu explicit veriyi koruyabilir ve onay formu için ön doldurabilir; bu alandan eksik diğer ücretleri tahmin ederek otomatik authoritative full-state policy üretmez. Test fixture'larının elle oluşturulan eşdeğer full-state'leri migration davranışı değildir. zamAylari yalnız legacy import adapter'ında okunur. Finalde compensation period Settings formundan ve normal motor input'undan çıkar.

## Production activation — ayrı kapı

Görev 4/5 veya kod entegrasyonunun tamamlanması activation değildir. Hepsi PASS olmalı:

- Legacy full parity, bağımsız formula goldens, N≥3 manual goldens; B-RND-01'in legacy yuvarlamayı koruyan kararının test kanıtı.
- Required compensation input snapshots, loader, transitif invalidation, FINALIZED guards/rollback.
- Native SQLite + browser IndexedDB/JSON V6; V5 restore baseline gate; fresh runtime rebind.
- Kullanıcının onayladığı 2026-09-15 baseline.
- Gerçek senaryo: DB’de yalnız 2026-09 ve sonrası; normal ve asgari GV opening doğru tax period ID’sine; gereken 2027 statutory/year tax parameters ve varsa açık başlangıç PEK carry.
- 2025-12, 2026-01, 2026-08 wage settings olmadan ve runtime zamAylari olmadan hesap; legitimate GV/PEK history ayrı authoritative kaynakta kalır.
- 2027-03-01 ve 2027-09-01 policy version, 15–14 cross, period deletion protection, retro, V5/V6 restore.
- Açık kalan finansal blocker olmamalı; acceptance evidence sunulmalı.

Bu görevde DB/sentetik bordro silinmedi. Gelecek acceptance isolated/safe test DB’de veya kullanıcı onaylı data ile yapılır.

## Requirement → owner → flow → success/failure → proof

| Gereksinim | Owner | Flow | PASS / failure / proof |
|---|---|---|---|
| Full-state model/resolver/timeline | 2 | Repo → index → core resolver | Baseline ve contiguous segments; invalid/missing baseline/fork fail; resolver + DB tests |
| Rounding ve full parity | 4, sonra 5 | Legacy calculate_payroll oracle + full shadow | Tüm financial Decimal projection compare; independent goldens ayrı; B-RND-01 parity kanıtı yoksa activation fail |
| Snapshot/source provenance | 3, sonra 5 | Inputs → payroll snapshot → persistence | Replayable current record; daily input yoksa fail; repo/backup fixtures |
| V6/V5 + runtime restore | 3, 8 | SQLite/JSON/IDB → strict validate → loader | V6 current replay; V5 timeline yokken calculate fail; partial/stale state fail |
| Timeline mutation closure | 5 | Transaction → graph impact → stale/block | Direct + transitive closure; finalized case rollback; graph/DB tests |
| Retro | 6 | Original snapshot → revision → allocation/carry/settlement | Paid/state linkage preserved; missing legacy inputs explicit fail; retro tests |
| Confirmed baseline UI | 7 | Raw prefill → user review → confirm → append | Confirm öncesi row yok; full-state exact Decimal; UI/save tests |
| Legacy cleanup + real case | 8 | Adapter → V6 → acceptance | No runtime old-wage dependency; DB/E2E evidence and version IDs |

### Kaynak index’i

[raise_transition.rs](../crates/payroll-core/src/raise_transition.rs#L41), [calculations.rs](../crates/payroll-core/src/calculations.rs#L989), [payroll_engine.rs](../crates/payroll-core/src/payroll_engine.rs#L2896), [transport_rates.rs](../crates/payroll-core/src/transport_rates.rs#L6), [models.rs](../crates/payroll-core/src/models.rs#L67), [policies.rs](../crates/payroll-core/src/policies.rs#L704), [validation.rs](../crates/payroll-core/src/validation.rs#L297), [migration_service.rs](../src-tauri/src/services/migration_service.rs#L19), [payrollPayload.ts](../src/services/storage/payrollPayload.ts#L924), [payrollPayloadSchema.ts](../src/services/storage/payrollPayloadSchema.ts#L1594), [decimalBoundary.ts](../src/services/payrollEngine/decimalBoundary.ts#L100), [periodDeletion.ts](../src/services/storage/periodDeletion.ts#L5). İncelenen dosyalarda policy timeline, compensation snapshot ve V6 kabiliyeti yoktur.
