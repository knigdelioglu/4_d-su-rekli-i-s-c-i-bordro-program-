declare module 'bun:test' {
  type DeepPartial<T> = T extends readonly unknown[]
    ? T
    : T extends object
      ? { [Key in keyof T]?: DeepPartial<T[Key]> }
      : T;

  type ContainedValue<T> = T extends string
    ? string
    : T extends readonly (infer Item)[]
      ? Item extends never ? unknown : Item
      : T extends ReadonlySet<infer Item>
        ? Item
        : unknown;

  type Expectation<T> = {
    toBe(expected: unknown): void;
    toEqual(expected: unknown): void;
    toMatchObject(expected: DeepPartial<T>): void;
    toContain(expected: ContainedValue<T>): void;
    toHaveLength(expected: number): void;
    toMatch(expected: string | RegExp): void;
    toThrow(expected?: string | RegExp): void;
    toBeNull(): void;
    toBeUndefined(): void;
    toBeDefined(): void;
    toBeTruthy(): void;
    toBeFalsy(): void;
    toBeCloseTo(expected: number, precision?: number): void;
    toBeGreaterThan(expected: number): void;
    toBeLessThan(expected: number): void;
    readonly not: Expectation<T>;
  } & (T extends PromiseLike<unknown>
    ? {
        readonly resolves: Expectation<Awaited<T>>;
        readonly rejects: Expectation<unknown>;
      }
    : {});

  export function describe(name: string, fn: () => void): void;
  export function test(name: string, fn: () => void | Promise<void>): void;
  export function expect<T>(actual: T): Expectation<T>;
}
