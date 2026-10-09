import test from 'node:test'
import assert from 'node:assert/strict'
import {modes,validateGroupConfiguration} from './group-configuration.js'
test('stored modes use the new discussion and Leader display names',()=>{
 assert.equal(modes.find(m=>m.id==='a2a').name,'讨论模式')
 assert.equal(modes.find(m=>m.id==='pmo').name,'Leader 模式')
 assert.match(modes.find(m=>m.id==='a2a').description,/职责|让出/)
})
test('complete group configuration validates modes, leader, order and limits',()=>{
 const value={name:'Team',policy:{mode:'pmo',members:[{path:['a'],role:'owner'}],leader:null,rounds:2,instructions:'plan'}}
 assert.match(validateGroupConfiguration(value),/Leader/)
 value.policy.leader=['a'];assert.equal(validateGroupConfiguration(value),'')
 value.policy.relay_strategy='random';assert.equal(validateGroupConfiguration(value),'')
 value.policy.members.push({path:['a','child'],role:'worker'});assert.match(validateGroupConfiguration(value),/重叠/)
})

test('simple chat requires exactly one Agent and permits automatic titles',()=>{
 const draft={name:'',policy:{mode:'chat',members:[{path:['worker'],role:'assistant'}],leader:null,rounds:1,instructions:''}}
 assert.equal(validateGroupConfiguration(draft),'')
 draft.policy.members.push({path:['second'],role:'reviewer'})
 assert.match(validateGroupConfiguration(draft),/一个 Agent/)
 draft.policy.members=[]
 assert.match(validateGroupConfiguration(draft),/一个 Agent/)
})
