/* eslint-disable @typescript-eslint/no-explicit-any */
/* eslint-disable @typescript-eslint/ban-types */

/**
 * Monstrous TypeScript Playground File
 * Intentionally overengineered for parser and tooling stress tests.
 */

/* ============================================================
 * 1. Type level madness
 * ============================================================
 */

export type Primitive =
  | string
  | number
  | boolean
  | bigint
  | symbol
  | null
  | undefined;

export type NonNullablePrimitive = Exclude<Primitive, null | undefined>;

// Conditional types and mapped types
export type DeepReadonly<T> = T extends Primitive | Function
  ? T
  : { readonly [K in keyof T]: DeepReadonly<T[K]> };

export type DeepPartial<T> = T extends Primitive | Function
  ? T | undefined
  : {
      [K in keyof T]?: DeepPartial<T[K]>;
    };

export type KeysMatching<T, V> = {
  [K in keyof T]: T[K] extends V ? K : never;
}[keyof T];

export type RequiredKeys<T> = {
  [K in keyof T]-?: undefined extends T[K] ? never : K;
}[keyof T];

export type OptionalKeys<T> = Exclude<keyof T, RequiredKeys<T>>;

export type Mutable<T> = {
  -readonly [K in keyof T]: T[K];
};

export type WithDefault<T, D> = [T] extends [never] ? D : T;

export type Merge<A, B> = {
  [K in keyof A | keyof B]: K extends keyof B
    ? B[K]
    : K extends keyof A
      ? A[K]
      : never;
};

// Template literal types
export type EventPhase = "start" | "progress" | "end" | "error";
export type EventCategory = "state" | "data" | "system";

export type EventName<
  Cat extends string,
  Name extends string,
  Phase extends EventPhase = "start",
> = `${Cat}:${Name}:${Phase}`;

export type StateName = "idle" | "running" | "paused" | "completed" | "failed";

export type StateEvent = EventName<"state", StateName, EventPhase>;

// Path type for nested objects
export type PathImpl<T, Prefix extends string = ""> = {
  [K in keyof T & (string | number)]: T[K] extends object
    ? PathImpl<T[K], `${Prefix}${K}.`> | `${Prefix}${K}`
    : `${Prefix}${K}`;
}[keyof T & (string | number)];

export type Path<T> = PathImpl<T>;

// Type driven event map
export interface BaseEventMap {
  "state:transition:start": { from: StateName; to: StateName };
  "state:transition:end": { from: StateName; to: StateName };
  "system:log": { level: "debug" | "info" | "warn" | "error"; message: string };
}

export type TypedEventMap<E> = E & BaseEventMap;

export type ListenerFn<T> = (payload: T) => void | Promise<void>;

export type ListenerMap<E> = {
  [K in keyof E]: Set<ListenerFn<E[K]>>;
};

export type MutableListenerMap<E> = {
  -readonly [K in keyof E]: Set<ListenerFn<E[K]>>;
};

/* ============================================================
 * 2. Interfaces with multiple extends and advanced constraints
 * ============================================================
 */

export interface Identified {
  readonly id: string;
}

export interface Timestamped {
  createdAt: Date;
  updatedAt: Date;
}

export interface Versioned {
  version: number;
}

export interface Persistable extends Identified, Timestamped, Versioned {
  isDirty(): boolean;
}

export interface Serializable<TFormat = unknown> {
  serialize(): TFormat;
}

export interface Hydratable<TFormat = unknown> {
  hydrate(source: TFormat): void;
}

export interface Repository<T extends Persistable> {
  save(entity: T): Promise<T>;
  findById(id: string): Promise<T | undefined>;
  findAll(): Promise<T[]>;
}

/* ============================================================
 * 3. Decorators
 * ============================================================
 */

type Constructor<T = object> = new (...args: any[]) => T;

function ClassLogger<TBase extends Constructor>(Base: TBase): TBase {
  return class extends Base {
    constructor(...args: any[]) {
      // Simple side effect for decorators
      super(...args);
    }
  };
}

function sealed<TBase extends Constructor>(Base: TBase): TBase {
  Object.seal(Base);
  Object.seal(Base.prototype);
  return Base;
}

function logCall(
  target: object,
  propertyKey: string | symbol,
  descriptor: TypedPropertyDescriptor<any>,
): TypedPropertyDescriptor<any> {
  const original = descriptor.value;
  descriptor.value = function (...args: any[]) {
    try {
      return original.apply(this, args);
    } catch (error) {
      throw error;
    }
  };
  return descriptor;
}

function memoize(
  target: object,
  propertyKey: string,
  descriptor: TypedPropertyDescriptor<(arg: unknown) => unknown>,
): TypedPropertyDescriptor<(arg: unknown) => unknown> {
  const original = descriptor.value!;
  const cache = new Map<unknown, unknown>();
  descriptor.value = function (arg: unknown) {
    if (cache.has(arg)) {
      return cache.get(arg);
    }
    const res = original.call(this, arg);
    cache.set(arg, res);
    return res;
  };
  return descriptor;
}

function lazy(
  target: any,
  propertyKey: string,
  descriptor: PropertyDescriptor,
) {
  const getter = descriptor.get;
  if (!getter) return;
  const cacheKey = Symbol(propertyKey);
  descriptor.get = function () {
    if (this[cacheKey] === undefined) {
      this[cacheKey] = getter.call(this);
    }
    return this[cacheKey];
  };
}

/* ============================================================
 * 4. Abstract classes, inheritance, state machines
 * ============================================================
 */

export type Transition<
  S extends string,
  E extends string,
  TContext = unknown,
> = {
  from: S;
  to: S;
  event: E;
  guard?: (context: TContext) => boolean;
  action?: (context: TContext) => void | Promise<void>;
};

export interface StateMachineSnapshot<S extends string, C> {
  state: S;
  context: DeepReadonly<C>;
}

export abstract class AbstractStateMachine<
  S extends string,
  E extends string,
  C,
> {
  protected currentState: S;
  protected readonly context: C;
  protected readonly transitionTable: Transition<S, E, C>[];

  constructor(initial: S, context: C, transitions: Transition<S, E, C>[]) {
    this.currentState = initial;
    this.context = context;
    this.transitionTable = transitions;
  }

  get snapshot(): StateMachineSnapshot<S, C> {
    return {
      state: this.currentState,
      context: this.context,
    } as const;
  }

  protected abstract onTransition(
    from: S,
    to: S,
    event: E,
    context: C,
  ): void | Promise<void>;

  async dispatch(event: E): Promise<void> {
    const candidates = this.transitionTable.filter(
      (t) => t.event === event && t.from === this.currentState,
    );
    for (const transition of candidates) {
      if (!transition.guard || transition.guard(this.context)) {
        if (transition.action) {
          await transition.action(this.context);
        }
        const from = this.currentState;
        this.currentState = transition.to;
        await this.onTransition(from, transition.to, event, this.context);
        return;
      }
    }
    throw new Error(
      `Invalid transition from "${this.currentState}" on "${String(event)}"`,
    );
  }
}

export type WorkflowState = StateName;
export type WorkflowEvent = "start" | "pause" | "resume" | "finish" | "fail";

export interface WorkflowContext {
  attempts: number;
  lastError?: Error;
}

export class WorkflowStateMachine extends AbstractStateMachine<
  WorkflowState,
  WorkflowEvent,
  WorkflowContext
> {
  constructor(ctx: WorkflowContext = { attempts: 0 }) {
    super("idle", ctx, [
      { from: "idle", event: "start", to: "running" },
      { from: "running", event: "pause", to: "paused" },
      { from: "paused", event: "resume", to: "running" },
      { from: "running", event: "finish", to: "completed" },
      {
        from: "running",
        event: "fail",
        to: "failed",
        action: (c) => c.attempts++,
      },
    ]);
  }

  protected async onTransition(
    from: WorkflowState,
    to: WorkflowState,
    event: WorkflowEvent,
    context: WorkflowContext,
  ): Promise<void> {
    // Placeholder for side effects
    if (event === "fail") {
      context.lastError = new Error(
        `Transitioned from ${from} to ${to} with failure`,
      );
    }
  }
}

/* ============================================================
 * 5. Event emitter with generics and symbol usage
 * ============================================================
 */

const internalListenersSymbol: unique symbol = Symbol("internalListeners");
const maxListenersSymbol: unique symbol = Symbol("maxListeners");

export interface TypedEventEmitter<E> {
  on<K extends keyof E & string>(event: K, listener: ListenerFn<E[K]>): this;
  once<K extends keyof E & string>(event: K, listener: ListenerFn<E[K]>): this;
  off<K extends keyof E & string>(event: K, listener: ListenerFn<E[K]>): this;
  emit<K extends keyof E & string>(event: K, payload: E[K]): Promise<void>;
}

@sealed
@ClassLogger
export class EventEmitter<E> implements TypedEventEmitter<E> {
  private [internalListenersSymbol]: MutableListenerMap<E>;
  private [maxListenersSymbol] = 50;

  constructor() {
    this[internalListenersSymbol] = {} as MutableListenerMap<E>;
  }

  setMaxListeners(count: number): this {
    this[maxListenersSymbol] = count;
    return this;
  }

  on<K extends keyof E & string>(event: K, listener: ListenerFn<E[K]>): this {
    const map = this.ensureSet(event);
    if (map.size >= this[maxListenersSymbol]) {
      throw new Error(`Max listeners reached for event "${event}"`);
    }
    map.add(listener as ListenerFn<E[keyof E]>);
    return this;
  }

  once<K extends keyof E & string>(event: K, listener: ListenerFn<E[K]>): this {
    const wrapper: ListenerFn<E[K]> = async (payload) => {
      this.off(event, wrapper);
      await listener(payload);
    };
    return this.on(event, wrapper);
  }

  off<K extends keyof E & string>(event: K, listener: ListenerFn<E[K]>): this {
    const map = this[internalListenersSymbol][event];
    if (map) {
      map.delete(listener as ListenerFn<E[keyof E]>);
    }
    return this;
  }

  async emit<K extends keyof E & string>(
    event: K,
    payload: E[K],
  ): Promise<void> {
    const map = this[internalListenersSymbol][event];
    if (!map || map.size === 0) return;
    const listeners = Array.from(map);
    const errors: unknown[] = [];
    for (const listener of listeners) {
      try {
        await listener(payload);
      } catch (err) {
        errors.push(err);
      }
    }
    if (errors.length > 0) {
      const aggregate = new AggregateError(
        errors,
        `Errors while emitting event "${event}"`,
      );
      throw aggregate;
    }
  }

  private ensureSet<K extends keyof E & string>(
    event: K,
  ): Set<ListenerFn<E[K]>> {
    if (!this[internalListenersSymbol][event]) {
      this[internalListenersSymbol][event] = new Set() as any;
    }
    return this[internalListenersSymbol][event] as any;
  }
}

/* ============================================================
 * 6. Builder, factory, singleton, strategy, observer
 * ============================================================
 */

export interface QueryCondition {
  field: string;
  operator: "=" | "!=" | ">" | "<" | ">=" | "<=" | "like";
  value: string | number | boolean;
}

export interface BuiltQuery {
  table: string;
  columns: string[];
  where: QueryCondition[];
  limit?: number;
  offset?: number;
}

export class QueryBuilder {
  private tableName = "";
  private columnsList: string[] = [];
  private whereConditions: QueryCondition[] = [];
  private limitValue?: number;
  private offsetValue?: number;

  table(name: string): this {
    this.tableName = name;
    return this;
  }

  select(...cols: string[]): this {
    this.columnsList.push(...cols);
    return this;
  }

  where(cond: QueryCondition): this {
    this.whereConditions.push(cond);
    return this;
  }

  limit(n: number): this {
    this.limitValue = n;
    return this;
  }

  offset(n: number): this {
    this.offsetValue = n;
    return this;
  }

  build(): BuiltQuery {
    if (!this.tableName) throw new Error("Table not set");
    return {
      table: this.tableName,
      columns: this.columnsList.length > 0 ? this.columnsList : ["*"],
      where: [...this.whereConditions],
      limit: this.limitValue,
      offset: this.offsetValue,
    };
  }
}

export type StrategyContext = {
  data: number[];
};

export interface Strategy {
  name: string;
  execute(ctx: StrategyContext): number[];
}

export class AscSortStrategy implements Strategy {
  name = "asc";
  execute(ctx: StrategyContext): number[] {
    return [...ctx.data].sort((a, b) => a - b);
  }
}

export class DescSortStrategy implements Strategy {
  name = "desc";
  execute(ctx: StrategyContext): number[] {
    return [...ctx.data].sort((a, b) => b - a);
  }
}

export class ShuffleStrategy implements Strategy {
  name = "shuffle";
  execute(ctx: StrategyContext): number[] {
    const copy = [...ctx.data];
    for (let i = copy.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      const tmp = copy[i];
      copy[i] = copy[j];
      copy[j] = tmp;
    }
    return copy;
  }
}

export class StrategyRegistry {
  private strategies = new Map<string, Strategy>();

  register(strategy: Strategy): this {
    this.strategies.set(strategy.name, strategy);
    return this;
  }

  get(name: string): Strategy | undefined {
    return this.strategies.get(name);
  }

  execute(name: string, ctx: StrategyContext): number[] {
    const strategy = this.get(name);
    if (!strategy) {
      throw new Error(`Strategy "${name}" not found`);
    }
    return strategy.execute(ctx);
  }
}

// Simple singleton config
export class AppConfig {
  private static _instance: AppConfig | undefined;

  private constructor(
    public readonly name: string,
    public readonly version: string,
    public readonly options: Record<string, unknown>,
  ) {}

  static get instance(): AppConfig {
    if (!this._instance) {
      this._instance = new AppConfig("MonstrousApp", "1.0.0", {});
    }
    return this._instance;
  }

  setOption<T>(key: string, value: T): void {
    this.options[key] = value;
  }

  getOption<T>(key: string, fallback?: T): T | undefined {
    return (this.options[key] as T | undefined) ?? fallback;
  }
}

// Observer pattern
export type Observer<T> = {
  next(value: T): void;
  error?(err: unknown): void;
  complete?(): void;
};

export class Observable<T> implements AsyncIterable<T> {
  private subscribers = new Set<Observer<T>>();

  subscribe(observer: Observer<T>): () => void {
    this.subscribers.add(observer);
    return () => this.subscribers.delete(observer);
  }

  next(value: T): void {
    for (const sub of this.subscribers) {
      sub.next(value);
    }
  }

  error(err: unknown): void {
    for (const sub of this.subscribers) {
      sub.error?.(err);
    }
  }

  complete(): void {
    for (const sub of this.subscribers) {
      sub.complete?.();
    }
    this.subscribers.clear();
  }

  async *[Symbol.asyncIterator](): AsyncIterator<T> {
    const queue: T[] = [];
    let done = false;
    let error: unknown;
    const observer: Observer<T> = {
      next(v) {
        queue.push(v);
      },
      error(errVal) {
        error = errVal;
        done = true;
      },
      complete() {
        done = true;
      },
    };
    const unsubscribe = this.subscribe(observer);
    try {
      while (!done || queue.length > 0) {
        if (error) throw error;
        if (queue.length === 0) {
          await new Promise((resolve) => setTimeout(resolve, 5));
          continue;
        }
        const value = queue.shift() as T;
        yield value;
      }
    } finally {
      unsubscribe();
    }
  }
}

/* ============================================================
 * 7. Proxies, WeakMap, WeakSet, iterators, generators
 * ============================================================
 */

const metaCache = new WeakMap<object, Record<string, unknown>>();
const observedObjects = new WeakSet<object>();

export function attachMeta(target: object, key: string, value: unknown): void {
  let meta = metaCache.get(target);
  if (!meta) {
    meta = {};
    metaCache.set(target, meta);
  }
  meta[key] = value;
}

export function getMeta(target: object, key: string): unknown {
  return metaCache.get(target)?.[key];
}

export function createLoggingProxy<T extends object>(target: T): T {
  observedObjects.add(target);
  const handler: ProxyHandler<T> = {
    get(obj, prop, receiver) {
      const value = Reflect.get(obj, prop, receiver);
      return value;
    },
    set(obj, prop, value, receiver) {
      const res = Reflect.set(obj, prop, value, receiver);
      return res;
    },
  };
  return new Proxy(target, handler);
}

// Iterable queue with generator based iterator
export class IterableQueue<T> implements Iterable<T> {
  private items: T[] = [];

  enqueue(item: T): void {
    this.items.push(item);
  }

  dequeue(): T | undefined {
    return this.items.shift();
  }

  *[Symbol.iterator](): Iterator<T> {
    for (const item of this.items) {
      yield item;
    }
  }
}

export function* range(
  start: number,
  end: number,
  step = 1,
): Generator<number, void, undefined> {
  for (let i = start; i <= end; i += step) {
    yield i;
  }
}

/* ============================================================
 * 8. Complex control flow, async functions, error handling
 * ============================================================
 */

export class RetryError extends Error {
  constructor(
    message: string,
    public readonly attempts: number,
    public readonly causeList: unknown[],
  ) {
    super(message);
  }
}

export async function retryAsync<T>(
  fn: () => Promise<T>,
  attempts: number,
  delayMs: number,
): Promise<T> {
  const errors: unknown[] = [];
  for (let i = 0; i < attempts; i++) {
    try {
      return await fn();
    } catch (err) {
      errors.push(err);
      if (i < attempts - 1) {
        await new Promise((resolve) => setTimeout(resolve, delayMs));
      }
    }
  }
  throw new RetryError("All retry attempts failed", attempts, errors);
}

// Nested functions and closures with complex control flow
export function makeComplexClosure(initial: number) {
  let counter = initial;

  function incrementBy(delta: number) {
    counter += delta;
    return counter;
  }

  function getSnapshot() {
    const current = counter;
    return {
      value: current,
      isPositive: current > 0,
      isEven: current % 2 === 0,
    };
  }

  function nestedLogic(value: number): "low" | "medium" | "high" {
    if (value < 10) {
      return "low";
    }
    if (value < 100) {
      return "medium";
    }
    return "high";
  }

  function complex(value: number) {
    const incremented = incrementBy(value);
    const snapshot = getSnapshot();
    const level = nestedLogic(incremented);
    return {
      incremented,
      snapshot,
      level,
    } as const;
  }

  return {
    incrementBy,
    getSnapshot,
    complex,
  };
}

/* ============================================================
 * 9. Function overloads
 * ============================================================
 */

export function deepClone<T>(value: T): T;
export function deepClone<T>(value: T, seen: WeakMap<any, any>): T;
export function deepClone(value: any, seen = new WeakMap<any, any>()): any {
  if (value === null || typeof value !== "object") {
    return value;
  }
  if (seen.has(value)) {
    return seen.get(value);
  }
  if (Array.isArray(value)) {
    const arr: any[] = [];
    seen.set(value, arr);
    for (const item of value) {
      arr.push(deepClone(item, seen));
    }
    return arr;
  }
  const cloned: any = {};
  seen.set(value, cloned);
  for (const key of Object.keys(value)) {
    cloned[key] = deepClone(value[key], seen);
  }
  return cloned;
}

/* ============================================================
 * 10. Utility functions with generics and constraints
 * ============================================================
 */

export function groupBy<T, K extends string | number | symbol>(
  items: readonly T[],
  keySelector: (item: T) => K,
): Record<K, T[]> {
  const result = {} as Record<K, T[]>;
  for (const item of items) {
    const key = keySelector(item);
    if (!result[key]) result[key] = [];
    result[key].push(item);
  }
  return result;
}

export function sortBy<T, K extends keyof T>(
  items: T[],
  key: K,
  direction: "asc" | "desc" = "asc",
): T[] {
  return [...items].sort((a, b) => {
    const av = a[key];
    const bv = b[key];
    if (av === bv) return 0;
    if (av == null) return 1;
    if (bv == null) return -1;
    if (av < bv) return direction === "asc" ? -1 : 1;
    return direction === "asc" ? 1 : -1;
  });
}

export function pick<T, K extends keyof T>(
  obj: T,
  keys: readonly K[],
): Pick<T, K> {
  const out = {} as Pick<T, K>;
  for (const key of keys) {
    if (key in obj) {
      out[key] = obj[key];
    }
  }
  return out;
}

export function omit<T, K extends keyof T>(
  obj: T,
  keys: readonly K[],
): Omit<T, K> {
  const set = new Set<keyof T>(keys as readonly (keyof T)[]);
  const out = {} as Omit<T, K>;
  for (const key in obj) {
    if (!set.has(key)) {
      (out as any)[key] = obj[key];
    }
  }
  return out;
}

/* ============================================================
 * 11. More exported helpers to stress test tools
 * ============================================================
 */

export async function* asyncCounter(
  limit: number,
  delayMs: number,
): AsyncGenerator<number, void, void> {
  for (let i = 0; i < limit; i++) {
    await new Promise((resolve) => setTimeout(resolve, delayMs));
    yield i;
  }
}

export async function collectAsyncIterable<T>(
  iterable: AsyncIterable<T>,
): Promise<T[]> {
  const result: T[] = [];
  for await (const item of iterable) {
    result.push(item);
  }
  return result;
}

export function createStatefulIterator<T>(items: T[]): Iterator<T> {
  let index = 0;
  return {
    next(): IteratorResult<T> {
      if (index < items.length) {
        return { done: false, value: items[index++] };
      }
      return { done: true, value: undefined as any };
    },
  };
}

export function isPersistable(value: unknown): value is Persistable {
  return (
    typeof value === "object" &&
    value !== null &&
    "id" in value &&
    "createdAt" in value &&
    "updatedAt" in value &&
    "version" in value &&
    typeof (value as Persistable).isDirty === "function"
  );
}

export async function mapAsync<T, R>(
  items: readonly T[],
  mapper: (item: T, index: number) => Promise<R>,
  concurrency = 4,
): Promise<R[]> {
  const result: R[] = new Array(items.length);
  let index = 0;
  let active = 0;

  return new Promise<R[]>((resolve, reject) => {
    const launchNext = () => {
      if (index >= items.length && active === 0) {
        resolve(result);
        return;
      }
      while (active < concurrency && index < items.length) {
        const current = index++;
        active++;
        mapper(items[current], current)
          .then((value) => {
            result[current] = value;
          })
          .catch((err) => {
            reject(err);
          })
          .finally(() => {
            active--;
            launchNext();
          });
      }
    };
    launchNext();
  });
}

/* ============================================================
 * 12. Complex exported type utilities for template types
 * ============================================================
 */

export type Join<
  Parts extends readonly string[],
  Sep extends string,
> = Parts extends []
  ? ""
  : Parts extends [infer Only extends string]
    ? Only
    : Parts extends [infer Head extends string, ...infer Tail extends string[]]
      ? `${Head}${Sep}${Join<Tail, Sep>}`
      : string;

export type DotPath<T> = Path<T>;

export type EventSignature<T extends string, Payload> = {
  type: T;
  payload: Payload;
};

export type CreateEventUnion<M> = {
  [K in keyof M]: EventSignature<K & string, M[K]>;
}[keyof M];

export type NarrowEvent<E, T extends string> = E extends { type: T }
  ? E
  : never;

/* ============================================================
 * 13. Example usages to exercise types
 * ============================================================
 */

export type ExampleEventMap = TypedEventMap<{
  "data:loaded:start": { count: number };
  "data:loaded:end": { count: number; duration: number };
}>;

export type ExampleEventUnion = CreateEventUnion<ExampleEventMap>;

export function handleExampleEvent(event: ExampleEventUnion): string {
  switch (event.type) {
    case "data:loaded:start":
      return `Loading ${event.payload.count} items`;
    case "data:loaded:end":
      return `Loaded ${event.payload.count} items in ${event.payload.duration}ms`;
    case "state:transition:start":
      return `Transition start ${event.payload.from} -> ${event.payload.to}`;
    case "state:transition:end":
      return `Transition end ${event.payload.from} -> ${event.payload.to}`;
    case "system:log":
      return `[${event.payload.level}] ${event.payload.message}`;
    default: {
      const exhaustive: never = event;
      return exhaustive;
    }
  }
}

/* ============================================================
 * 14. Class with decorators, async methods, nested functions
 * ============================================================
 */

export interface CalculationResult {
  input: number;
  output: number;
  meta: {
    isPrime: boolean;
    factors: number[];
  };
}

@sealed
@ClassLogger
export class MonstrousCalculator {
  private cache = new Map<number, CalculationResult>();

  @logCall
  @memoize
  expensiveCheck(value: unknown): boolean {
    if (typeof value !== "number") return false;
    if (!Number.isFinite(value) || value <= 1) return false;
    for (let i = 2; i * i <= value; i++) {
      if (value % i === 0) return false;
    }
    return true;
  }

  @logCall
  async calculate(value: number): Promise<CalculationResult> {
    if (this.cache.has(value)) {
      return this.cache.get(value)!;
    }

    // Nested helper
    const factorize = (v: number): number[] => {
      const factors: number[] = [];
      for (let i = 1; i <= v; i++) {
        if (v % i === 0) factors.push(i);
      }
      return factors;
    };

    const isPrime = this.expensiveCheck(value);
    const factors = factorize(value);
    const output = factors.reduce((a, b) => a + b, 0);

    const result: CalculationResult = {
      input: value,
      output,
      meta: {
        isPrime,
        factors,
      },
    };
    this.cache.set(value, result);
    return result;
  }

  @lazy
  get stats() {
    const entries = [...this.cache.values()];
    const sum = entries.reduce((acc, cur) => acc + cur.output, 0);
    return {
      count: entries.length,
      average: entries.length === 0 ? 0 : sum / entries.length,
    };
  }
}

/* ============================================================
 * 15. Even more exports for good measure
 * ============================================================
 */

export function createDefaultStrategyRegistry(): StrategyRegistry {
  return new StrategyRegistry()
    .register(new AscSortStrategy())
    .register(new DescSortStrategy())
    .register(new ShuffleStrategy());
}

export function applyStrategyToArray(
  values: number[],
  strategyName: string,
): number[] {
  const registry = createDefaultStrategyRegistry();
  return registry.execute(strategyName, { data: values });
}

export function createWorkflowMachine(): WorkflowStateMachine {
  return new WorkflowStateMachine();
}

export function buildSimpleQuery(table: string, ids: number[]): BuiltQuery {
  return new QueryBuilder()
    .table(table)
    .select("id", "name", "createdAt")
    .where({ field: "id", operator: "in" as any, value: ids.length })
    .build();
}

export async function simulateEvents<E>(
  emitter: EventEmitter<E>,
  events: { type: keyof E & string; payload: E[keyof E] }[],
): Promise<void> {
  for (const e of events) {
    await emitter.emit(e.type as any, e.payload as any);
  }
}

export function observeQueue<T>(queue: IterableQueue<T>): Observable<T> {
  const observable = new Observable<T>();
  for (const item of queue) {
    observable.next(item);
  }
  observable.complete();
  return observable;
}

export function createRangeArray(start: number, end: number): number[] {
  return [...range(start, end)];
}

export function getConfigOption<T>(key: string, fallback?: T): T | undefined {
  return AppConfig.instance.getOption(key, fallback);
}

export function setConfigOption<T>(key: string, value: T): void {
  AppConfig.instance.setOption(key, value);
}

export function exportAllKeys<T extends object>(obj: T): (keyof T)[] {
  return Object.keys(obj) as (keyof T)[];
}

export function getPath<T>(obj: T, path: DotPath<T>): unknown {
  const segments = path.split(".");
  let current: any = obj;
  for (const seg of segments) {
    if (current == null) return undefined;
    current = current[seg];
  }
  return current;
}

export function setPath<T extends object>(
  obj: T,
  path: DotPath<T>,
  value: unknown,
): void {
  const segments = path.split(".");
  let current: any = obj;
  for (let i = 0; i < segments.length; i++) {
    const seg = segments[i];
    if (i === segments.length - 1) {
      current[seg] = value;
      return;
    }
    if (!current[seg] || typeof current[seg] !== "object") {
      current[seg] = {};
    }
    current = current[seg];
  }
}
