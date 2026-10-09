import {readState} from './state-fixture.mjs'
import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,rm,readFile} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {start,stop,request,modelFixture,act,pause,policy,history} from './admin-fixture.mjs'
test('shared definitions resolve all four binding layers, inherit, persist and isolate scope',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-bindings-')),work=await mkdtemp(join(tmpdir(),'crabot-binding-work-')),model=await modelFixture();let server
 try{
  server=await start(dir,{...model.env,AGENT_WORKDIR:work})
  const p=(await request(server,'/v1/repl')).projects[0].id
  const created=await act(server,p,[{name:'project_create',input:{name:'Second',space_name:'Second'}},{name:'agent_start',input:{client_id:'worker',role:'tester',provider:'mock'}}])
  const q=created.outputs[0].project_id
  assert.ok(q)
  await act(server,q,[{name:'agent_start',input:{client_id:'worker',role:'tester',provider:'mock'}}])
  const put=async(path,value,status=200)=>{const r=await fetch(server.url+path,{method:'PUT',headers:{'x-admin-token':server.token,'content-type':'application/json'},body:JSON.stringify(value)});assert.equal(r.status,status,await r.clone().text());return r.json()}
  const base=project=>'/v1/repl/'+project+'/capabilities/business/'
  const definition={id:'shared-guide',description:'Shared guide',enabled:true,allow_python:false,files:{'SKILL.md':'shared instructions'}}
  const saved=await put(base(p)+'skill',{expected_version:0,definition})
  const row=async(project=p,kind='skill',id=saved.id)=>(await request(server,base(project)+kind+'?agent=worker')).rows.find(r=>r.resource.id===id)
  assert.equal((await row(q)).resolution.enabled,true)
  const bind=async(layer,enabled,project=p,kind='skill',id=saved.id)=>{const r=await row(project,kind,id),v=r.resolution.layers.find(l=>l.layer===layer);return put(base(project)+kind+'/bindings?agent=worker',{id,layer,enabled,expected_version:v.version})}
  await bind('global',false);assert.equal((await row(q)).resolution.enabled,false)
  await bind('project',true);assert.equal((await row()).resolution.enabled,true);assert.equal((await row(q)).resolution.enabled,false)
  await bind('agent',false);assert.equal((await row()).resolution.enabled,false)
  await bind('project_agent',true);assert.equal((await row()).resolution.enabled,true);assert.equal((await row()).resolution.source,'project_agent')
  await bind('project_agent',null);assert.equal((await row()).resolution.enabled,false)
  await bind('agent',null);assert.equal((await row()).resolution.enabled,true)
  const testTool=async(project,name,args)=>{const run=await request(server,'/v1/repl/'+project+'/tool-config/business/worker/tests',{name,arguments:args});for(let i=0;i<100;i++){const r=await request(server,'/v1/repl/'+project+'/tool-tests/'+run.id);if(r.status!=='running')return r;await pause(10)}throw Error('timeout')}
  assert.equal((await testTool(p,'find',{target:'skill',id:'shared-guide'})).output.skills[0].instructions,'shared instructions')
  const management=await request(server,'/v1/repl/'+p+'/capabilities/management/skill?agent=admin')
  assert.ok(!management.rows.some(r=>r.resource.definition.id==='shared-guide'))
  const toolId='builtin:business:tool:find'
  await bind('global',false,p,'tool',toolId)
  const denied=await fetch(server.url+'/v1/repl/'+p+'/tool-config/business/worker/tests',{method:'POST',headers:{'x-admin-token':server.token,'content-type':'application/json'},body:JSON.stringify({name:'find',arguments:{target:'tool'}})})
  assert.equal(denied.status,400)
  await bind('project_agent',true,p,'tool',toolId)
  assert.equal((await testTool(p,'find',{target:'tool'})).status,'completed')
  await put(base(q)+'skill',{id:saved.id,expected_version:1,definition:{...definition,files:{'SKILL.md':'updated everywhere'}}})
  assert.equal((await row()).resource.definition.files['SKILL.md'],'updated everywhere')
  await put(base(p)+'skill',{id:saved.id,expected_version:1,definition},409)
  await put(base(p)+'skill',{expected_version:0,definition:{...definition,id:'unsafe',allow_python:true}},400)
  await put(base(p)+'tool',{expected_version:0,definition:{name:'find',description:'bad',command:'echo bad',enabled:true}},400)
  const command=await put(base(p)+'tool',{expected_version:0,definition:{name:'shared_echo',description:'Shared command',command:'printf shared',enabled:false}})
  assert.equal((await row(q,'tool',command.id)).resource.definition.command,'printf shared')
  // A project in the Web is a collaboration group, not the legacy storage namespace.
  const a=await request(server,'/v1/repl/'+p+'/groups',{name:'Project A',policy:policy('worker')})
  const b=await request(server,'/v1/repl/'+p+'/groups',{name:'Project B',policy:policy('worker')})
  const index=await request(server,'/v1/repl')
  assert.ok(index.collaboration_projects.some(g=>g.key===a.key&&g.namespace_id===p))
  const groupRow=async(group,kind='skill',id=saved.id)=>(await request(server,base(p)+kind+'?agent=worker&group='+group)).rows.find(r=>r.resource.id===id)
  assert.equal((await groupRow(a.key)).resolution.enabled,true,'inherits legacy project binding')
  await put(base(p)+'skill/bindings?agent=worker&group='+a.key,{id:saved.id,layer:'project',enabled:false,expected_version:0})
  assert.equal((await groupRow(a.key)).resolution.enabled,false)
  assert.equal((await groupRow(b.key)).resolution.enabled,true)
  const run=await request(server,'/v1/repl/'+p+'/groups/'+a.key+'/messages',{content:'Check project context'})
  await history(server,p,run.id)
  const stored=await readState(dir)
  const records=Object.values(stored.collections.runs)
  const workerRun=records.find(r=>r.client_id==='worker'&&r.content?.includes('Check project context'))
  assert.ok(workerRun,'local worker ran')
  assert.ok(!workerRun.skills.some(s=>s.id==='shared-guide'),'execution respects project Skill binding')
  await put(base(p)+'tool/bindings?agent=worker&group='+a.key,{id:toolId,layer:'project_agent',enabled:false,expected_version:0})
  const deniedGroup=await fetch(server.url+'/v1/repl/'+p+'/tool-config/business/worker/tests',{method:'POST',headers:{'x-admin-token':server.token,'content-type':'application/json'},body:JSON.stringify({name:'find',arguments:{target:'tool'},group:a.key})})
  assert.equal(deniedGroup.status,400)
  const allowedGroup=await request(server,'/v1/repl/'+p+'/tool-config/business/worker/tests',{name:'find',arguments:{target:'tool'},group:b.key})
  assert.ok(allowedGroup.id)
  await stop(server);server=await start(dir,{...model.env,AGENT_WORKDIR:work})
  assert.equal((await row()).resolution.enabled,true)
  assert.equal((await groupRow(a.key)).resolution.enabled,false)
  assert.equal((await groupRow(b.key)).resolution.enabled,true)
  assert.equal((await row(q)).resolution.enabled,false)
  await put(base(p)+'skill',{id:saved.id,expected_version:2,deleted:true})
  assert.equal(await row(q),undefined)
 }finally{await stop(server);await model.close();await rm(dir,{recursive:true,force:true});await rm(work,{recursive:true,force:true})}
})
