"""Smoke tests for the Python bindings."""
import json
import patchlang_python

# Test 1: Simple parse
result = json.loads(patchlang_python.parse('instance FOH is CL5'))
assert len(result['errors']) == 0, f"Expected no errors, got {result['errors']}"
assert len(result['program']['statements']) == 1
stmt = result['program']['statements'][0]
assert stmt['type'] == 'Instance'
assert stmt['name'] == 'FOH'
assert stmt['template_name'] == 'CL5'
print('PASS: simple instance')

# Test 2: Validate
assert patchlang_python.validate('instance FOH is CL5') is True
assert patchlang_python.validate('!!! garbage') is False
print('PASS: validate')

# Test 3: Parse real fixture
with open('tests/fixtures/examples/worship-venue.patch') as f:
    worship = f.read()
result = json.loads(patchlang_python.parse(worship))
assert len(result['errors']) == 0, f"Errors: {result['errors']}"
types = {}
for s in result['program']['statements']:
    t = s.get('type', 'Error')
    types[t] = types.get(t, 0) + 1
assert types['Template'] == 3, f"Expected 3 templates, got {types.get('Template')}"
assert types['Instance'] == 4, f"Expected 4 instances, got {types.get('Instance')}"
print('PASS: worship-venue.patch')

# Test 4: Hillsong MTG
with open('tests/fixtures/examples/hillsong-mtg.patch') as f:
    hillsong = f.read()
result = json.loads(patchlang_python.parse(hillsong))
assert len(result['errors']) == 0, f"Errors: {result['errors']}"
types = {}
for s in result['program']['statements']:
    t = s.get('type', 'Error')
    types[t] = types.get(t, 0) + 1
assert types['Template'] == 24, f"Expected 24 templates, got {types.get('Template')}"
assert types['Instance'] == 53, f"Expected 53 instances, got {types.get('Instance')}"
assert types['Connect'] == 99, f"Expected 99 connects, got {types.get('Connect')}"
print('PASS: hillsong-mtg.patch (1485 lines, 203 statements)')

# Test: ProgramBuilder.set_label forwards label properties (#32)
#
# `set_label` hard-coded an empty properties map, so the Python binding could not set
# `phantom`, `capsule`, the insert legs (#31), or any custom key — while the WASM
# binding had accepted them all along. This is the first test to exercise the builder
# class from Python at all.
builder = patchlang_python.ProgramBuilder.from_source("""
template Desk {
  ports {
    Mic_In[1..8]: in(XLR)
    Ext_Out[1..16]: out(XLR)
  }
}
instance FOH is Desk {}
""")

builder.set_label('FOH', 'Mic_In', 1, 'Kick', {
    'phantom': 'true',
    'insert_send': 'Ext_Out[3], Ext_Out[10]',
})
out = builder.format()
assert 'phantom' in out, f"properties must reach the emitted patch, got:\n{out}"
assert 'insert_send' in out, f"insert legs must reach the emitted patch, got:\n{out}"
assert 'Ext_Out[3], Ext_Out[10]' in out, f"leg list must survive verbatim, got:\n{out}"

# Back-compat: the no-props call must still work, since this is a published wheel.
builder.set_label('FOH', 'Mic_In', 2, 'Snare')
out2 = builder.format()
assert 'Snare' in out2, f"omitting props must still set the label, got:\n{out2}"
print('PASS: set_label forwards properties (#32)')

print('\nAll Python tests passed!')
