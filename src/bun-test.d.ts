declare module 'bun:test' {
  export function describe(name: string, fn: () => void): void;
  export function test(name: string, fn: () => void | Promise<void>): void;
  export function expect<T>(actual: T): {
    toBe(expected: unknown): void;
    toEqual(expected: unknown): void;
    toThrow(expected?: string | RegExp): void;
    toBeTruthy(): void;
    toBeFalsy(): void;
    toBeCloseTo(expected: number, precision?: number): void;
    toBeGreaterThan(expected: number): void;
    toBeLessThan(expected: number): void;
    not: {
      toBe(expected: unknown): void;
      toEqual(expected: unknown): void;
      toThrow(expected?: string | RegExp): void;
      toBeTruthy(): void;
      toBeFalsy(): void;
      toBeCloseTo(expected: number, precision?: number): void;
      toBeGreaterThan(expected: number): void;
      toBeLessThan(expected: number): void;
    };
  };
}
