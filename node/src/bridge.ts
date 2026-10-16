/** Internal conversion policy. Not exported as a package subpath. */
export type ErrorCode =
  | 'STORAGE'
  | 'REPOSITORY'
  | 'GIT'
  | 'IDENTITY'
  | 'SIGNER'
  | 'COB'
  | 'READ_ONLY'
  | 'INVALID_DATA'
  | 'INTERNAL';
export class RadicleError extends Error {
  readonly code: ErrorCode;
  constructor(code: ErrorCode, message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = 'RadicleError';
    this.code = code;
  }
}
const codes = new Set<string>([
  'STORAGE',
  'REPOSITORY',
  'GIT',
  'IDENTITY',
  'SIGNER',
  'COB',
  'READ_ONLY',
  'INVALID_DATA',
  'INTERNAL',
]);
function translate(error: unknown): Error {
  if (error instanceof RadicleError) return error;
  const message = error instanceof Error ? error.message : String(error);
  const match = /^RADICLE:([A-Z_]+):([\s\S]*)$/.exec(message);
  if (match?.[1] === 'ARG_VALUE') {
    return Object.assign(new TypeError(match[2], { cause: error }), {
      code: 'ERR_INVALID_ARG_VALUE',
    });
  }
  if (match?.[1] === 'ARG_RANGE') {
    return Object.assign(new RangeError(match[2], { cause: error }), { code: 'ERR_OUT_OF_RANGE' });
  }
  // Object conversion (eg. null options) can throw a JS TypeError without a
  // napi status code. Keep argument errors out of the INTERNAL error category.
  if (
    error instanceof TypeError ||
    (error instanceof Error &&
      'code' in error &&
      typeof error.code === 'string' &&
      (error.code === 'InvalidArg' || error.code.endsWith('Expected')))
  ) {
    return Object.assign(new TypeError(message, { cause: error }), {
      code: 'ERR_INVALID_ARG_TYPE',
    });
  }
  if (match && codes.has(match[1])) {
    return new RadicleError(match[1] as ErrorCode, match[2], { cause: error });
  }
  return new RadicleError('INTERNAL', message, { cause: error });
}
export function sync<T>(call: () => T): T {
  try {
    return call();
  } catch (e) {
    throw translate(e);
  }
}
export async function run<T>(call: () => Promise<T>): Promise<T> {
  try {
    return freeze(await call());
  } catch (e) {
    throw translate(e);
  }
}
/** Freeze detached DTO containers, never typed arrays or native handles. */
function freeze<T>(value: T): T {
  if (value === null || typeof value !== 'object' || ArrayBuffer.isView(value)) return value;
  if (Array.isArray(value) || Object.getPrototypeOf(value) === Object.prototype) {
    for (const item of Object.values(value)) freeze(item);
    Object.freeze(value);
  }
  return value;
}
