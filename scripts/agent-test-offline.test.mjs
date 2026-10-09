import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {start,stop,request,history,policy} from './admin-fixture.mjs'

test('test chat runs a stopped local Agent without enabling normal work',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-offline-test-'));let server
 try{
  server=await start(dir,{ADMIN_AGENT_PROVIDER:'mock'})
  const p=(await request(server,'/v1/repl')).projects[0].id,base=`/v1/repl/${p}`
  const saved=await fetch(server.url+base+'/agents',{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({client_id:'offline',provider:'mock',role:'test fixture',expected_version:0})})
  assert.equal(saved.status,200)
  const online=async()=> (await request(server,base+'/agents/candidates')).agents.find(a=>a.id==='offline').online
  assert.equal(await online(),false)
  const group=await request(server,base+'/agent-tests',{path:['offline']})
  const run=await request(server,base+'/groups/'+group.key+'/messages',{content:'offline test'})
  const events=await history(server,p,run.id)
  assert.ok(events.some(e=>e.type==='agent.message'),JSON.stringify(events))
  assert.ok(!events.some(e=>e.type==='agent.error'),JSON.stringify(events))
  assert.equal(await online(),false)
  const ordinary=await request(server,base+'/groups',{policy:{...policy('offline'),mode:'chat'}})
  const rejected=await request(server,base+'/groups/'+ordinary.key+'/messages',{content:'not a test'})
  assert.ok((await history(server,p,rejected.id)).some(e=>e.type==='agent.error'))
 }finally{await stop(server);await rm(dir,{recursive:true,force:true})}
})
