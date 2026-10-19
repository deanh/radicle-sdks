import type { RawOpData } from '../dist/native.cjs';
import { RadicleError } from './bridge.js';

/** Integer tokens are exact; decimal/exponent tokens use finite IEEE-754 numbers. */
export type RawJsonValue =
  | null
  | boolean
  | string
  | number
  | bigint
  | ReadonlyArray<RawJsonValue>
  | { readonly [key: string]: RawJsonValue };

/** Detached, frozen raw entry. Byte views are independent mutable copies. */
export interface RawOp extends RawOpData {
  /** Decode the current action byte views; no storage reads or writes. */
  readonly actionsJson: () => ReadonlyArray<RawJsonValue>;
}
// ignoreBOM keeps the BOM in decoded text, so JSON.parse rejects it just as
// serde_json does; the option disables BOM stripping, not BOM validation.
const decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
const integer = /^-?(?:0|[1-9][0-9]*)$/;
const badSurrogate = /[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/;

/** Internal adapter, not a public native conversion factory. */
export function rawOp(data: RawOpData): RawOp {
  return Object.freeze({
    ...data,
    actionsJson(): ReadonlyArray<RawJsonValue> {
      return Object.freeze(
        data.actions.map((bytes, index) => {
          try {
            return JSON.parse(
              decoder.decode(bytes),
              (key: string, value: unknown, context?: { source?: string }): unknown => {
                // Source exists for primitives only, not object/array reviver visits.
                if (typeof value === 'number') {
                  if (!context?.source) throw new Error('JSON number source unavailable');
                  if (integer.test(context.source) && !Number.isSafeInteger(value))
                    return BigInt(context.source);
                  // Like Python's serde_json decoder, do not accept float overflow.
                  if (!Number.isFinite(value)) throw new Error('JSON float outside finite range');
                }
                // Match Rust JSON strings: no invalid UTF-8 or unpaired surrogate escapes.
                if (
                  badSurrogate.test(key) ||
                  (typeof value === 'string' && badSurrogate.test(value))
                )
                  throw new Error('unpaired JSON surrogate');
                if (value !== null && typeof value === 'object') return Object.freeze(value);
                return value;
              },
            ) as RawJsonValue;
          } catch (cause) {
            throw new RadicleError(
              'COB',
              `action #${index} of entry ${data.id} is not valid supported JSON`,
              { cause },
            );
          }
        }),
      );
    },
  });
}
