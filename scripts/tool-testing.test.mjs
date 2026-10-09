import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,rm} from 'node:fs/promises'
import {join} from 'node:path'
import {tmpdir} from 'node:os'
import {start,stop,request,modelFixture,act,pause} from './admin-fixture.mjs'
test('tool testing uses real scoped registry and cannot skip approvals or disabled tools',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-tool-test-')),workdir=await mkdtemp(join(tmpdir(),'crabot-tool-work-')),model=await modelFixture();let server
 try{
  server=await start(dir,{...model.env,AGENT_WORKDIR:workdir});const p=(await request(server,'/v1/repl')).projects[0].id
  await act(server,p,[{name:'agent_start',input:{client_id:'worker',role:'worker',provider:'mock'}}])
  const base=`/v1/repl/${p}`,headers={'x-admin-token':server.token,'content-type':'application/json'}
  const put=async(path,body)=>{const r=await fetch(server.url+path,{method:'PUT',headers,body:JSON.stringify(body)});assert.ok(r.ok,await r.clone().text());return r.json()}
  const wait=async id=>{for(let i=0;i<200;i++){const row=await request(server,base+'/tool-tests/'+id);if(row.status!=='running')return row;await pause(15)}throw Error('test timed out')}
  let run=await request(server,base+'/tool-config/business/worker/tests',{name:'find',arguments:{target:'tool'}})
  const listing=await wait(run.id)
  assert.equal(listing.status,'completed',JSON.stringify(listing))
  const definition={id:'guide',description:'Guide',enabled:true,allow_python:false,files:{'SKILL.md':'# Test instructions','scripts/test.py':'print(1)'}}
  await put(base+'/skills/business',{expected_version:0,definition})
  run=await request(server,base+'/tool-config/business/worker/tests',{name:'find',arguments:{target:'skill',id:'guide'}})
  assert.equal((await wait(run.id)).output.skills[0].instructions,'# Test instructions')
  const retired=await fetch(server.url+base+'/tool-config/business/worker/tests',{method:'POST',headers,body:JSON.stringify({name:'python_run',arguments:{skill_id:'guide'}})})
  assert.equal(retired.status,400)
  await put(base+'/tool-config/business/worker',{expected_version:0,policy:{disabled:[],external:[{name:'fixture_echo',description:'test output',command:'printf test-ok',enabled:true}]}})
  run=await request(server,base+'/tool-config/business/worker/tests',{name:'fixture_echo',arguments:{}})
  let approval
  for(let i=0;i<100;i++){approval=(await request(server,'/v1/workspace/approvals')).requests.find(r=>r.correlation_id===run.id);if(approval)break;await pause(10)}
  assert.equal(approval.command,'printf test-ok')
  assert.equal((await request(server,base+'/tool-tests/'+run.id)).status,'running')
  await request(server,'/v1/workspace/approvals/'+approval.id,{allow:true})
  assert.equal((await wait(run.id)).output.stdout,'test-ok')
  run=await request(server,base+'/tool-config/business/worker/tests',{name:'fixture_echo',arguments:{}})
  await fetch(server.url+base+'/tool-tests/'+run.id,{method:'DELETE',headers})
  assert.equal((await wait(run.id)).status,'cancelled')
  assert.ok(!(await request(server,'/v1/workspace/approvals')).requests.some(r=>r.correlation_id===run.id))
  await put(base+'/tool-config/business/worker',{expected_version:1,policy:{disabled:['shell'],external:[]}})
  const denied=await fetch(server.url+base+'/tool-config/business/worker/tests',{method:'POST',headers,body:JSON.stringify({name:'shell',arguments:{command:'pwd'}})})
  assert.equal(denied.status,400)
  assert.equal((await fetch(server.url+base+'/tool-tests/'+run.id,{headers:{origin:'https://evil.example'}})).status,403)
 }finally{await stop(server);await model.close();await rm(dir,{recursive:true,force:true});await rm(workdir,{recursive:true,force:true})}
})
