import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {start,stop,request,history,pause,policy} from './admin-fixture.mjs'
test('managed Agents, single chat, virtual composition, auto titles and remote Crabot roundtrip',async()=>{
 const dirs=await Promise.all([mkdtemp(join(tmpdir(),'crabot-directory-a-')),mkdtemp(join(tmpdir(),'crabot-directory-b-'))])
 let a,b
 try{
  a=await start(dirs[0],{ADMIN_AGENT_PROVIDER:'mock'});b=await start(dirs[1],{ADMIN_AGENT_PROVIDER:'mock'})
  const p=(await request(a,'/v1/repl')).projects[0].id,q=(await request(b,'/v1/repl')).projects[0].id
  const base=(p)=>'/v1/repl/'+p
  const put=async(server,path,body,status=200)=>{const r=await fetch(server.url+path,{method:'PUT',headers:{'x-admin-token':server.token,'content-type':'application/json'},body:JSON.stringify(body)});assert.equal(r.status,status,await r.clone().text());return r.json()}
  const rejected=async(server,path,body)=>{const r=await fetch(server.url+path,{method:'POST',headers:{'x-admin-token':server.token,'content-type':'application/json'},body:JSON.stringify(body)});assert.equal(r.status,400,await r.text())}
  for(const [s,n] of [[a,p],[b,q]]) {
   await put(s,base(n)+'/agents',{client_id:'worker',role:'tester',provider:'mock',expected_version:0})
   assert.equal((await request(s,base(n)+'/agents')).agents.find(a=>a.id==='worker').online,false)
   await request(s,base(n)+'/agents/worker/start',{})
  }
  await put(a,base(p)+'/agents',{client_id:'worker',role:'changed',provider:'mock',expected_version:1},400)
  await rejected(a,base(p)+'/groups',{policy:policy('imaginary')})
  await rejected(a,base(p)+'/groups',{policy:{...policy('worker'),mode:'chat',members:[{path:['worker'],role:''},{path:['missing'],role:''}]}})
  await rejected(a,base(p)+'/virtual-agents',{name:'Not a composition',expected_version:0,policy:{...policy('worker'),mode:'chat'}})
  const virtual=await request(a,base(p)+'/virtual-agents',{name:'Virtual team',expected_version:0,policy:policy('worker')})
  await put(a,base(p)+'/virtual-agents',{id:virtual.key,name:'cycle',expected_version:virtual.version,policy:policy(virtual.key)},400)
  const chat=await request(a,base(p)+'/groups',{policy:{...policy(virtual.key),mode:'chat'}})
  assert.equal(chat.body.name,'新聊天')
  const run=await request(a,base(p)+'/groups/'+chat.key+'/messages',{content:'讨论第一版发布计划'})
  const events=await history(a,p,run.id)
  assert.ok(events.some(e=>e.type==='agent.message'&&e.payload.content.includes('Mock')))
  let updated=(await request(a,base(p)+'/groups')).find(g=>g.key===chat.key)
  assert.equal(updated.body.name,'讨论第一版发布计划')
  await put(a,base(p)+'/groups/'+chat.key+'/configuration',{name:'手动标题',policy:updated.body.policy,expected_version:updated.version})
  const next=await request(a,base(p)+'/groups/'+chat.key+'/messages',{content:'第二轮问题'})
  await history(a,p,next.id)
  assert.equal((await request(a,base(p)+'/groups')).find(g=>g.key===chat.key).body.name,'手动标题')
  const childVirtual=await request(b,base(q)+'/virtual-agents',{name:'Remote team',expected_version:0,policy:policy('worker')})
  const mount=await request(b,base(q)+'/peers/mount',{url:a.url})
  assert.equal(mount.status,'pending')
  await request(b,'/v1/admin-agent/'+q+'/approvals/'+mount.id,{allow:true})
  let remoteAgent
  for(let i=0;i<160;i++){remoteAgent=(await request(a,base(p)+'/agents/candidates')).agents.find(x=>x.kind==='remote'&&x.path[1]===childVirtual.key);if(remoteAgent)break;await pause(50)}
  assert.ok(remoteAgent,'SSE child Agent discovery established')
  const remote=await request(a,base(p)+'/groups',{name:'Remote project',policy:{...policy('unused'),mode:'chat',members:[{path:remoteAgent.path,role:'remote team'}]}})
  const remoteRun=await request(a,base(p)+'/groups/'+remote.key+'/messages',{content:'Remote virtual Agent task'})
  const remoteEvents=await history(a,p,remoteRun.id)
  assert.ok(remoteEvents.some(e=>e.type==='agent.message'&&e.payload.content.includes('Mock')),JSON.stringify(remoteEvents))
  assert.match(remoteAgent.remote_address,/127\.0\.0\.1:\d+/)
  const uplink=(await request(b,base(q)+'/peers')).find(r=>r.direction==='upstream')
  const downlink=(await request(a,base(p)+'/peers')).find(r=>r.direction==='downstream')
  assert.equal(uplink.status,'connected');assert.ok(downlink.remote_address)
  await put(b,base(q)+'/peers',{name:uplink.name,direction:'upstream',disabled:true})
  await pause(600)
  assert.equal((await request(b,base(q)+'/peers'))[0].status,'disconnected')
  assert.ok(!(await request(a,base(p)+'/agents/candidates')).agents.some(x=>x.kind==='remote'))
  await stop(b);b=await start(dirs[1],{ADMIN_AGENT_PROVIDER:'mock'})
  assert.equal((await request(b,base(q)+'/peers'))[0].status,'disconnected')
  await put(b,base(q)+'/peers',{name:uplink.name,direction:'upstream',disabled:false})
  for(let i=0;i<100;i++){if((await request(b,base(q)+'/peers'))[0].status==='connected')break;await pause(50)}
  assert.equal((await request(b,base(q)+'/peers'))[0].status,'connected')
  await put(a,base(p)+'/peers',{name:downlink.name,direction:'downstream',disabled:true})
  await pause(600)
  assert.ok(!(await request(a,base(p)+'/agents/candidates')).agents.some(x=>x.kind==='remote'))
  await put(a,base(p)+'/peers',{name:downlink.name,direction:'downstream',disabled:false})
  const listing=JSON.stringify(await request(b,base(q)+'/agents'))
  assert.ok(!listing.includes('sk_hash')&&!listing.includes('registration_secret'))
  const test=await request(a,base(p)+'/agent-tests',{path:['default']})
  assert.equal(test.body.kind,'agent_test')
  assert.equal((await request(a,base(p)+'/agent-tests',{path:['default']})).key,test.key)
  assert.ok(!(await request(a,'/v1/repl')).collaboration_projects.some(g=>g.key===test.key))
  const testRun=await request(a,base(p)+'/groups/'+test.key+'/messages',{content:'Default agent test'})
  assert.ok((await history(a,p,testRun.id)).some(e=>e.type==='agent.message'&&e.payload.content.includes('Mock')))
  const immutable=await fetch(a.url+base(p)+'/agents/'+encodeURIComponent(remoteAgent.id),{method:'DELETE',headers:{'content-type':'application/json'},body:JSON.stringify({expected_version:0})})
  assert.equal(immutable.status,400)
  await put(a,base(p)+'/agents',{client_id:'disposable',role:'test deletion',provider:'mock',expected_version:0})
  const removed=await fetch(a.url+base(p)+'/agents/disposable',{method:'DELETE',headers:{'content-type':'application/json'},body:JSON.stringify({expected_version:1})})
  assert.equal(removed.status,200,await removed.text())
  const pending=await request(a,base(p)+'/agents/worker/stop',{})
  await request(a,'/v1/admin-agent/'+p+'/approvals/'+pending.id,{allow:true})
  await put(a,base(p)+'/agents',{client_id:'worker',role:'updated tester',provider:'mock',expected_version:1})
  await stop(a);a=await start(dirs[0],{ADMIN_AGENT_PROVIDER:'mock'})
  const directory=await request(a,base(p)+'/agents')
  assert.ok(directory.agents.some(x=>x.id===virtual.key&&x.kind==='virtual'))
  assert.equal(directory.agents.find(x=>x.id==='worker').role,'updated tester')
 }finally{await stop(b);await stop(a);await Promise.all(dirs.map(d=>rm(d,{recursive:true,force:true})))}
})
