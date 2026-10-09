import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {start,stop,request,history} from './admin-fixture.mjs'
import {conversationView} from '../apps/web/src/conversation-view.js'
test('A2A index retains each member and round',{timeout:30000},async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-member-view-'));let server
 try {
  server=await start(dir,{ADMIN_AGENT_PROVIDER:'mock'})
  const p=(await request(server,'/v1/repl')).projects[0].id,base='/v1/repl/'+p
  const response=await fetch(server.url+base+'/agents',{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({client_id:'second',role:'reviewer',provider:'mock',response_instructions:'Only the key facts.',expected_version:0})})
  assert.equal(response.status,200,await response.clone().text())
  await request(server,base+'/agents/second/start',{})
  const group=await request(server,base+'/groups',{policy:{mode:'a2a',members:[{path:['default'],role:'writer'},{path:['second'],role:'reviewer'}],rounds:2,leader:null,instructions:''}})
  const run=await request(server,base+'/groups/'+group.key+'/messages',{content:'Discuss briefly'})
  await history(server,p,run.id)
  const logs=await request(server,base+'/chats/'+group.key+'/history')
  const replies=conversationView(logs.events).filter(r=>r.type==='assistant')
  assert.equal(replies.length,4,JSON.stringify(logs.events))
  assert.deepEqual(replies.map(r=>r.agent),['default','second','default','second'])
  assert.equal(new Set(replies.map(r=>r.invocation_id)).size,4)
  assert.ok(logs.events.some(e=>e.type==="agent.message"))
  const agents=(await request(server,base+'/agents')).agents
  assert.equal(agents.find(a=>a.id==='second').response_instructions,'Only the key facts.')
 } finally {await stop(server);await rm(dir,{recursive:true,force:true})}
})
