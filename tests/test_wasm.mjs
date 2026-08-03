// Quick smoke test for the WASM package.
//
// Assertions go through `node:assert`, NOT `console.assert`. `console.assert` only
// prints — it does not throw and does not set a non-zero exit code — so every check
// in this file was decorative: the suite passed no matter what broke, in test-all.sh
// and in CI. Same family as #36: a check that cannot fail reports success forever.
import assert from 'node:assert/strict'
import { parse, validate, load_from_patch } from '../pkg-node/patchlang_wasm.js'
import { readFileSync } from 'fs'

// Test 1: Simple parse
const result = JSON.parse(parse('instance FOH is CL5'))
assert.ok(result.errors.length === 0, 'Expected no errors')
assert.ok(result.program.statements.length === 1, 'Expected 1 statement')
assert.ok(result.program.statements[0].type === 'Instance', 'Expected Instance')
assert.ok(result.program.statements[0].name === 'FOH', 'Expected FOH')
console.log('PASS: simple instance')

// Test 2: Validate
assert.ok(validate('instance FOH is CL5') === true, 'Expected valid')
assert.ok(validate('!!! garbage') === false, 'Expected invalid')
console.log('PASS: validate')

// Test 3: Parse real fixture file
const worship = readFileSync('tests/fixtures/examples/worship-venue.patch', 'utf-8')
const worshipResult = JSON.parse(parse(worship))
assert.ok(worshipResult.errors.length === 0, 'Expected no errors for worship-venue')
const types = {}
for (const s of worshipResult.program.statements) {
  types[s.type] = (types[s.type] || 0) + 1
}
assert.ok(types.Template === 3, `Expected 3 templates, got ${types.Template}`)
assert.ok(types.Instance === 4, `Expected 4 instances, got ${types.Instance}`)
console.log('PASS: worship-venue.patch')

// Test 4: Parse Hillsong MTG (1485 lines)
const hillsong = readFileSync('tests/fixtures/examples/hillsong-mtg.patch', 'utf-8')
const hillsongResult = JSON.parse(parse(hillsong))
assert.ok(hillsongResult.errors.length === 0, `Expected 0 errors, got ${hillsongResult.errors.length}`)
const hTypes = {}
for (const s of hillsongResult.program.statements) {
  hTypes[s.type] = (hTypes[s.type] || 0) + 1
}
assert.ok(hTypes.Template === 24, `Expected 24 templates, got ${hTypes.Template}`)
assert.ok(hTypes.Instance === 53, `Expected 53 instances, got ${hTypes.Instance}`)
assert.ok(hTypes.Connect === 99, `Expected 99 connects, got ${hTypes.Connect}`)
console.log('PASS: hillsong-mtg.patch (1485 lines, 203 statements)')

// Test 5: quoted strings survive the round trip (#35, D026).
// The original corruption was demonstrated through this WASM boundary, so it is
// pinned here and not only in the Rust suite.
const quoted = `
template T { ports { P: out(XLR) } }
instance D is T {
  bus Mix {
    label: "The \\"Big\\" Mix"
    input: P[1]
  }
}
config D {
  label P[1]: "Lead \\"Vox\\" Mic"
}
`
const quotedLoaded = JSON.parse(load_from_patch(quoted, ''))
const qInst = quotedLoaded.instances[0]
assert.equal(qInst.internal_buses[0].display_name, 'The "Big" Mix',
  'bus display_name must survive quotes')
assert.equal(qInst.channel_labels['P'][0].label, 'Lead "Vox" Mic',
  'channel label must survive quotes')
console.log('PASS: quoted strings round-trip (#35)')

console.log('\nAll WASM tests passed!')
