//! Pure payment-event exemption allocation. Inputs come from statutory
//! entitlement and authoritative prior snapshots, never an accrual type.
use crate::calculations::round_gv_amount;
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GvExemptionState {
    pub monthly_entitlement: Decimal,
    pub used_before: Decimal,
    pub remaining_before: Decimal,
    pub applied_current: Decimal,
    pub remaining_after: Decimal,
    pub withheld_income_tax: Decimal,
}

impl GvExemptionState {
    pub fn resolve(monthly_entitlement: Decimal, used_before: Decimal, gross_tax: Decimal) -> Self {
        let monthly_entitlement = round_gv_amount(monthly_entitlement);
        let used_before = round_gv_amount(used_before);
        let gross_tax = round_gv_amount(gross_tax).max(Decimal::ZERO);
        let remaining_before = (monthly_entitlement - used_before).max(Decimal::ZERO);
        let applied_current = gross_tax.min(remaining_before);
        Self {
            monthly_entitlement,
            used_before,
            remaining_before,
            applied_current,
            remaining_after: remaining_before - applied_current,
            withheld_income_tax: gross_tax - applied_current,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn august_three_payment_events_share_entitlement() {
        let first = GvExemptionState::resolve(dec!(5615.10), dec!(0), dec!(2000));
        assert_eq!(first.remaining_before, dec!(5615.10));
        assert_eq!(first.applied_current, dec!(2000));
        assert_eq!(first.remaining_after, dec!(3615.10));
        assert_eq!(first.withheld_income_tax, dec!(0));
        let second = GvExemptionState::resolve(dec!(5615.10), first.applied_current, dec!(10000));
        assert_eq!(second.used_before, dec!(2000));
        assert_eq!(second.remaining_before, dec!(3615.10));
        assert_eq!(second.applied_current, dec!(3615.10));
        assert_eq!(second.remaining_after, dec!(0));
        assert_eq!(second.withheld_income_tax, dec!(6384.90));
        let third = GvExemptionState::resolve(dec!(5615.10), dec!(5615.10), dec!(2000));
        assert_eq!(third.applied_current, dec!(0));
        assert_eq!(third.remaining_before, dec!(0));
    }
}
