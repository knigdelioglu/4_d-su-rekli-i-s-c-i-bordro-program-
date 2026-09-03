declare module 'payroll-wasm-generated' {
  const init: () => Promise<unknown>;
  export default init;
  export function calculate_payroll_json(requestJson: string): string;
  export function finalize_payroll_json(requestJson: string): string;
  export function validate_payroll_json(requestJson: string): void;
  export function evaluate_mutation_policy_json(requestJson: string): string;
  export function get_payroll_notices_json(requestJson: string): string;
}
