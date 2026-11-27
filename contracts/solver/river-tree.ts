import { z } from "zod";
import { riverInputSchema } from "./engine.ts";

// Internal engine contract only. Product real-analysis/1 remains restricted river.
export const riverActionSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.enum(["check", "fold", "call"]) }).strict(),
  z
    .object({
      kind: z.enum(["bet", "raise", "all-in"]),
      amount: z.number().int().min(1).max(1_000_000),
    })
    .strict(),
]);
export const explicitRiverSchema = riverInputSchema
  .omit({ sizes: true })
  .extend({
    schemaVersion: z.literal(2),
    limits: riverInputSchema.shape.limits
      .extend({
        maxMemoryBytes: z.number().int().min(1).max(268_435_456),
        maxSeconds: z.number().int().min(1).max(120),
      })
      .strict(),
    treeLines: z.array(z.array(riverActionSchema).min(1).max(16)).max(128),
    locks: z
      .array(
        z
          .object({
            path: z.array(z.number().int().nonnegative()).max(16),
            allocations: z.record(
              z.string().regex(/^(?:[2-9TJQKA][cdhs]){2}$/),
              z.array(z.number().finite().min(0).max(1)).min(1).max(129),
            ),
          })
          .strict(),
      )
      .max(128)
      .default([]),
  })
  .strict();

export const nativeQuerySchema = z
  .object({
    schemaVersion: z.literal(1),
    nativeSha256: z.string().regex(/^[a-f0-9]{64}$/),
    inputSha256: z
      .string()
      .regex(/^[a-f0-9]{64}$/)
      .optional(),
    path: z.array(z.number().int().nonnegative()).max(16),
    player: z.union([z.literal(0), z.literal(1)]),
    offset: z.number().int().min(0).max(1326),
    limit: z.number().int().min(1).max(100),
    maxMemoryBytes: z.number().int().min(1).max(268_435_456),
  })
  .strict();
export type ExplicitRiverInput = z.infer<typeof explicitRiverSchema>;
export type NativeQuery = z.infer<typeof nativeQuerySchema>;

export const nativePageSchema = z
  .object({
    schemaVersion: z.literal(1),
    nativeSha256: z.string().regex(/^[a-f0-9]{64}$/),
    path: z.array(z.number().int().nonnegative()).max(16),
    terminal: z.boolean(),
    actingPlayer: z.union([z.literal(0), z.literal(1)]).nullable(),
    player: z.union([z.literal(0), z.literal(1)]),
    actions: z.array(z.string()).max(129),
    invested: z.tuple([
      z.number().int().nonnegative(),
      z.number().int().nonnegative(),
    ]),
    unit: z.literal("centibb"),
    total: z.number().int().min(0).max(1326),
    offset: z.number().int().min(0).max(1326),
    nextOffset: z.number().int().min(0).max(1326).nullable(),
    rows: z
      .array(
        z
          .object({
            engineIndex: z.number().int().min(0).max(1325),
            combo: z.string().regex(/^(?:[2-9TJQKA][cdhs]){2}$/),
            ownPathWeight: z.number().finite().nonnegative(),
            compatibleWeight: z.number().finite().nonnegative(),
            equity: z.number().finite().nullable(),
            ev: z.number().finite().nullable(),
            strategy: z
              .array(z.number().finite().min(0).max(1))
              .max(129)
              .nullable(),
            actionEv: z.array(z.number().finite()).max(129).nullable(),
          })
          .strict(),
      )
      .max(100),
  })
  .strict();
