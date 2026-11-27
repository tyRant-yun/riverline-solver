import { z } from "zod";

export const engineProtocol = "riverline-engine/1" as const;
export const engineCommit = "9d1509fe5077d019825f833eed04b16d342dfda1" as const;
const card = z.string().regex(/^[2-9TJQKA][cdhs]$/);
const weights = z.record(
  z.string().regex(/^(?:[2-9TJQKA][cdhs]){2}$/),
  z.number().finite().min(0).max(1),
);
export const riverInputSchema = z
  .object({
    schemaVersion: z.literal(1),
    caseId: z.string().min(1).max(200),
    board: z.array(card).length(5),
    pot: z.number().int().min(1).max(1_000_000),
    stack: z.number().int().min(1).max(1_000_000),
    ranges: z.tuple([weights, weights]),
    sizes: z.tuple([
      z.tuple([z.string().max(9), z.literal("")]),
      z.tuple([z.literal(""), z.literal("")]),
    ]),
    limits: z
      .object({
        maxIterations: z.number().int().min(1).max(100_000),
        targetExploitability: z.number().finite().nonnegative(),
        maxMemoryBytes: z.number().int().min(1).max(2_147_483_648),
        maxSeconds: z.number().int().min(1).max(600),
      })
      .strict(),
    locks: z
      .array(
        z
          .object({
            path: z.array(z.number().int().nonnegative()).max(3),
            allocations: z.record(
              z.string(),
              z.array(z.number().finite().min(0).max(1)).max(2),
            ),
          })
          .strict(),
      )
      .max(3)
      .default([]),
  })
  .strict();
export type RiverInput = z.infer<typeof riverInputSchema>;
export const engineIdentitySchema = z
  .object({
    requestId: z.string().regex(/^[A-Za-z0-9_-]{1,128}$/),
    jobId: z.string().regex(/^[A-Za-z0-9_-]{1,128}$/),
    attempt: z.number().int().positive(),
    specHash: z.string().regex(/^[a-f0-9]{64}$/),
  })
  .strict();
export type EngineIdentity = z.infer<typeof engineIdentitySchema>;
export const engineCommandSchema = z
  .object({
    protocolVersion: z.literal(engineProtocol),
    operation: z.enum(["solve", "cancel"]),
    identity: engineIdentitySchema,
  })
  .strict();
export const engineEventSchema = z
  .object({
    protocolVersion: z.literal(engineProtocol),
    kind: z.enum([
      "hello",
      "started",
      "progress",
      "completed",
      "cancelled",
      "failed",
    ]),
    identity: engineIdentitySchema.nullable(),
    seq: z.number().int().nonnegative(),
    payload: z.record(z.string(), z.unknown()),
  })
  .strict();
export type EngineEvent = z.infer<typeof engineEventSchema>;
