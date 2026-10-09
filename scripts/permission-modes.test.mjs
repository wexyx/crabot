import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,rm,readFile} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {start,stop,request,modelFixture,policy,history,pause} from './admin-fixture.mjs'

test('permission modes are explicit, persisted, scoped and enforce command approvals',{timeout:60000},async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-permissions-')),model=await modelFixture();let server
 try{
  server=await start(join(dir,'data'),model.env)
  const p=(await request(server,'/v1/repl')).projects[0].id,base=`/v1/repl/${p}`
  const path=base+'/agents/default/permissions',listPath='/v1/permissions/allowlist'
  const putList=async body=>{const r=await fetch(server.url+listPath,{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify(body)});assert.equal(r.status,200,await r.clone().text());return r.json()}
  const put=async(body,status=200)=>{const r=await fetch(server.url+path,{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify(body)});assert.equal(r.status,status,await r.clone().text());return r.json()}
  let current=await request(server,path);assert.equal(current.mode,'ask')
  const denied=await fetch(server.url+path,{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({mode:'full',expected_version:current.version})});assert.ok(!denied.ok);assert.equal((await request(server,path)).mode,'ask')
  current=await put({mode:'auto',expected_version:current.version})
  assert.equal(current.mode,'auto')
  let list=await request(server,listPath);assert.ok(list.command_allowlist.length>=40)
  list=await putList({expected_version:list.version,command_allowlist:[...list.command_allowlist,'printf custom-allowed']})
  const invalid=await fetch(server.url+listPath,{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({expected_version:list.version,command_allowlist:['pwd; id']})});assert.ok(!invalid.ok)
  assert.ok((await request(server,listPath)).command_allowlist.includes('printf custom-allowed'))
  const stale=await fetch(server.url+path,{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({mode:'ask',expected_version:0})});assert.equal(stale.status,409)
  const group=await request(server,base+'/groups',{policy:{...policy('default'),mode:'chat'}})
  const send=async command=>request(server,`${base}/groups/${group.key}/messages`,{content:'TEST_PLAN:'+JSON.stringify([{name:'shell',input:{command}}])})
  const customRun=await send('printf custom-allowed')
  assert.match(JSON.stringify(await history(server,p,customRun.id)),/Fixture complete/)
  const autoRun=await send('pwd')
  assert.match(JSON.stringify(await history(server,p,autoRun.id)),/Fixture complete/)
  assert.equal((await request(server,'/v1/workspace/approvals')).requests.length,0)
  // Ambiguous/non-allowlisted commands still require a human in auto mode.
  const guarded=await send('printf permission-test')
  let approvals=[]
  for(let i=0;i<100;i++){approvals=(await request(server,'/v1/workspace/approvals')).requests;if(approvals.length)break;await pause(25)}
  assert.equal(approvals.length,1)
  await request(server,'/v1/workspace/approvals/'+approvals[0].id,{allow:false})
  await history(server,p,guarded.id)
  current=await put({mode:'full',expected_version:current.version,confirm_full_access:true})
  const outside=join(dir,'outside-proof.txt')
  const fullRun=await send(`printf full-access-fixture > '${outside}'`)
  await history(server,p,fullRun.id)
  assert.equal(await readFile(outside,'utf8'),'full-access-fixture')
  await stop(server);server=await start(join(dir,'data'),model.env)
  current=await request(server,path);assert.equal(current.mode,'full');assert.ok((await request(server,listPath)).command_allowlist.includes('printf custom-allowed'))
  current=await put({mode:'ask',expected_version:current.version})
  const askRun=await send('pwd')
  for(let i=0;i<100;i++){approvals=(await request(server,'/v1/workspace/approvals')).requests;if(approvals.length)break;await pause(25)}
  assert.equal(approvals.length,1)
  assert.equal(approvals[0].conversation_id,p+':'+group.key)
  await request(server,'/v1/workspace/approvals/'+approvals[0].id,{allow:true,conversation:true});await history(server,p,askRun.id)
  const grantedRun=await send('printf conversation-allowed');await history(server,p,grantedRun.id)
  assert.equal((await request(server,'/v1/workspace/approvals')).requests.length,0)
  const other=await request(server,base+'/groups',{policy:{...policy('default'),mode:'chat'}})
  const otherRun=await request(server,base+'/groups/'+other.key+'/messages',{content:'TEST_PLAN:'+JSON.stringify([{name:'shell',input:{command:'pwd'}}])})
  for(let i=0;i<100;i++){approvals=(await request(server,'/v1/workspace/approvals')).requests;if(approvals.length)break;await pause(25)}
  assert.equal(approvals.length,1);assert.equal(approvals[0].conversation_id,p+':'+other.key)
  await request(server,'/v1/workspace/approvals/'+approvals[0].id,{allow:false});await history(server,p,otherRun.id)
  await request(server,base+'/groups/'+group.key+'/commands',{command:'/new'})
  const resetRun=await send('pwd')
  for(let i=0;i<100;i++){approvals=(await request(server,'/v1/workspace/approvals')).requests;if(approvals.length)break;await pause(25)}
  assert.equal(approvals.length,1)
  await request(server,'/v1/workspace/approvals/'+approvals[0].id,{allow:false});await history(server,p,resetRun.id)
 }finally{await stop(server);await model.close();await rm(dir,{recursive:true,force:true})}
})
