import { z } from "zod";
import { riverInputSchema } from "./engine.ts";
export const turnPathSchema = z
  .array(
    z.discriminatedUnion("kind", [
      z
        .object({
          kind: z.literal("action"),
          index: z.number().int().min(0).max(128),
        })
        .strict(),
      z
        .object({
          kind: z.literal("deal"),
          card: z.string().regex(/^[2-9TJQKA][cdhs]$/),
        })
        .strict(),
    ]),
  )
  .max(24);
export type TurnPath = z.infer<typeof turnPathSchema>;
export const turnProfileSchema = z
  .object({
    kind: z.enum(["hu-turn/3", "hu-turn/4", "hu-flop/5"]),
    flopBets: z
      .tuple([
        z.number().int().nonnegative().max(1000000),
        z.number().int().nonnegative().max(1000000),
      ])
      .optional(),
    turnBets: z.tuple([
      z.number().int().nonnegative().max(1000000),
      z.number().int().nonnegative().max(1000000),
    ]),
    riverBets: z.tuple([
      z.number().int().nonnegative().max(1000000),
      z.number().int().nonnegative().max(1000000),
    ]),
  })
  .strict();
export type TurnProfile = z.infer<typeof turnProfileSchema>;
export const turnInputSchema = riverInputSchema
  .omit({ sizes: true })
  .extend({
    schemaVersion: z.union([z.literal(3), z.literal(4)]),
    board: riverInputSchema.shape.board.element.array().length(4),
    turnBets: turnProfileSchema.shape.turnBets,
    riverBets: turnProfileSchema.shape.riverBets,
    locks: z.tuple([]).default([]),
    limits: riverInputSchema.shape.limits
      .extend({
        maxMemoryBytes: z.number().int().min(1).max(268435456),
        maxSeconds: z.number().int().min(1).max(120),
      })
      .strict(),
  })
  .strict();
export type TurnInput = z.infer<typeof turnInputSchema>;
export const flopInputSchema = turnInputSchema
  .extend({
    schemaVersion: z.literal(5),
    board: riverInputSchema.shape.board.element.array().length(3),
    flopBets: z.tuple([
      z.number().int().nonnegative().max(1000000),
      z.number().int().nonnegative().max(1000000),
    ]),
  })
  .strict();
export type FlopInput = z.infer<typeof flopInputSchema>;
export const turnNodeSchema = z
  .object({
    path: turnPathSchema,
    privateCards: z.tuple([
      z.array(z.string()).max(16),
      z.array(z.string()).max(16),
    ]),
    kind: z.enum(["decision", "chance", "terminal"]),
    board: z
      .array(z.string().regex(/^[2-9TJQKA][cdhs]$/))
      .min(3)
      .max(5),
    terminal: z.boolean(),
    player: z.union([z.literal(0), z.literal(1)]).nullable(),
    actions: z.array(z.string()).max(3),
    chanceCards: z
      .array(
        z
          .object({
            card: z.string().regex(/^[2-9TJQKA][cdhs]$/),
            probability: z.number().finite().min(0).max(1).nullable(),
          })
          .strict(),
      )
      .max(48),
    invested: z.tuple([
      z.number().int().nonnegative(),
      z.number().int().nonnegative(),
    ]),
    weights: z.tuple([
      z.array(z.number().finite().nonnegative()).max(16),
      z.array(z.number().finite().nonnegative()).max(16),
    ]),
    compatibleWeights: z.tuple([
      z.array(z.number().finite().nonnegative()).max(16),
      z.array(z.number().finite().nonnegative()).max(16),
    ]),
    equity: z.tuple([
      z.array(z.number().finite()).max(16),
      z.array(z.number().finite()).max(16),
    ]),
    ev: z.tuple([
      z.array(z.number().finite()).max(16),
      z.array(z.number().finite()).max(16),
    ]),
    actionEv: z.array(z.number().finite()).max(48),
    strategy: z.array(z.number().finite().min(0).max(1.000001)).max(48),
  })
  .strict();
export type TurnNode = z.infer<typeof turnNodeSchema>;
export const turnPageQuerySchema = z
  .object({
    schemaVersion: z.union([z.literal(4), z.literal(5)]),
    nativeSha256: z.string().regex(/^[a-f0-9]{64}$/),
    inputSha256: z.string().regex(/^[a-f0-9]{64}$/),
    path: turnPathSchema,
    maxMemoryBytes: z.number().int().min(1).max(268435456),
    player: z.union([z.literal(0), z.literal(1)]),
    indices: z.array(z.number().int().min(0).max(127)).max(100),
  })
  .strict();
export const turnPageSchema = z
  .object({
    schemaVersion: z.union([z.literal(4), z.literal(5)]),
    path: turnPathSchema,
    player: z.union([z.literal(0), z.literal(1)]),
    counts: z.tuple([
      z.number().int().min(1).max(128),
      z.number().int().min(1).max(128),
    ]),
    node: turnNodeSchema.pick({
      kind: true,
      board: true,
      terminal: true,
      player: true,
      actions: true,
      chanceCards: true,
      invested: true,
    }),
    rows: z
      .array(
        z
          .object({
            index: z.number().int().min(0).max(127),
            combo: z.string().regex(/^(?:[2-9TJQKA][cdhs]){2}$/),
            weight: z.number().finite().nonnegative(),
            compatibleMass: z.number().finite().nonnegative(),
            equity: z.number().finite(),
            ev: z.number().finite(),
            strategy: z
              .array(z.number().finite().min(0).max(1.000001))
              .max(3)
              .nullable(),
            actionEv: z.array(z.number().finite()).max(3).nullable(),
          })
          .strict(),
      )
      .max(100),
    aggregate: z
      .object({
        denominator: z.number().finite().nonnegative(),
        frequencies: z
          .array(
            z
              .object({
                action: z.string(),
                frequency: z.number().finite().min(0).max(1.000001).nullable(),
              })
              .strict(),
          )
          .max(3),
        classes: z
          .array(
            z
              .object({
                hand: z.string(),
                probability: z.number().finite().min(0).max(1).nullable(),
              })
              .strict(),
          )
          .max(169),
      })
      .strict(),
  })
  .strict();
export type TurnPage = z.infer<typeof turnPageSchema>;
