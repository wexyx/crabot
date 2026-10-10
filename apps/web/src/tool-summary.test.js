import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
import {toolSummary,toolEventSummary} from './tool-summary.js'

const cases=JSON.parse(readFileSync(new URL('../../../tests/fixtures/tool-summaries.json',import.meta.url),'utf8'))
test('Web and CLI use the same concise tool summaries',()=>{
 for(const c of cases)assert.equal(toolSummary(c.name,c.input),c.expected)
})
test('summaries fit one short Unicode-safe line and leave inputs intact',()=>{
 const input={target:'history',query:'查🙂'.repeat(100)}
 const before=JSON.stringify(input)
 const summary=toolSummary('find',input)
 assert.equal(Array.from(summary).length,56)
 assert.ok(summary.endsWith('…'))
 assert.ok(!summary.includes('\uFFFD'))
 assert.equal(JSON.stringify(input),before)
})
test('both management and group event envelopes expose tool arguments',()=>{
 const input={target:'skill',id:'browser-automation'}
 for(const row of [
  {name:'find',input},
  {content:JSON.stringify({name:'find',arguments:input})},
  {payload:{content:JSON.stringify({name:'find',input})}},
 ])assert.equal(toolEventSummary(row),'find · skill : "browser-automation"')
 assert.equal(toolEventSummary({content:'invalid'}),'tool')
})
