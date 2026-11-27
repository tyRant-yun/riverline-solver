import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { z } from 'zod';
import { engineCommandSchema, engineEventSchema, riverInputSchema } from '../contracts/solver/engine.ts';
import { explicitRiverSchema, nativeQuerySchema, nativePageSchema } from '../contracts/solver/river-tree.ts';
import { turnInputSchema, flopInputSchema, turnNodeSchema, turnPathSchema, turnPageSchema, turnPageQuerySchema } from '../contracts/solver/turn.ts';
const schemas = { command: engineCommandSchema, event: engineEventSchema, river: riverInputSchema, 'river-tree': explicitRiverSchema, 'native-query': nativeQuerySchema, 'native-page': nativePageSchema, turn: turnInputSchema, flop: flopInputSchema, 'turn-node': turnNodeSchema, 'turn-path': turnPathSchema, 'turn-page': turnPageSchema, 'turn-page-query': turnPageQuerySchema };
for (const [name, schema] of Object.entries(schemas)) assert.deepEqual(z.toJSONSchema(schema), JSON.parse(readFileSync(new URL(`../contracts/solver/${name}.schema.json`, import.meta.url), 'utf8')), name);
console.log(`${Object.keys(schemas).length} frozen engine schemas match their TypeScript sources`);
