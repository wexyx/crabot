import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {modelFixture,start,stop,request,policy,history} from './admin-fixture.mjs'

test('local Agents own model settings; default editor reads live settings and secrets stay masked',{timeout:30000},async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-agent-model-')),model=await modelFixture();let server
 const configuration={MODEL_PROVIDER:'compatible',MODEL_API:'chat',MODEL_NAME:'private-worker',MODEL_BASE_URL:model.env.MODEL_BASE_URL,MODEL_API_KEY:'private-worker-secret'}
 try{
  server=await start(dir,{ADMIN_AGENT_PROVIDER:'mock'})
  const p=(await request(server,'/v1/repl')).projects[0].id,base='/v1/repl/'+p
  async function put(path,body){const response=await fetch(server.url+path,{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify(body)});assert.equal(response.status,200,await response.clone().text());return response.json()}
  let saved=await put(base+'/agents',{client_id:'configured',provider:'crabot',role:'角色定义',expected_version:0,configuration})
  assert.ok(!JSON.stringify(saved).includes(configuration.MODEL_API_KEY))
  saved=await put(base+'/agents',{client_id:'configured',provider:'crabot',role:'独立模型',expected_version:1,configuration:{MODEL_NAME:'edited-model'}})
  const directory=await request(server,base+'/agents')
  const local=directory.agents.find(a=>a.id==='configured')
  assert.equal(local.configuration.values.MODEL_NAME,'edited-model')
  assert.equal(local.configuration.has_api_key,true)
  assert.ok(!JSON.stringify(directory).includes(configuration.MODEL_API_KEY))
  await request(server,base+'/agents/configured/start',{})
  const group=await request(server,base+'/groups',{policy:{...policy('configured'),mode:'chat'}})
  const run=await request(server,base+'/groups/'+group.key+'/messages',{content:'TEST_PLAN:[]'})
  assert.match(JSON.stringify(await history(server,p,run.id)),/Fixture complete/)
  await put('/v1/admin-agent/configuration',{...configuration,ADMIN_AGENT_PROVIDER:'crabot'})
  const active=await request(server,'/v1/admin-agent/configuration')
  assert.equal(active.configuration.values.ADMIN_AGENT_PROVIDER,'crabot')
  assert.equal(active.configuration.has_api_key,true)
  assert.ok(!JSON.stringify(active).includes(configuration.MODEL_API_KEY))
  const defaultAgent=(await request(server,base+'/agents')).agents.find(a=>a.id==='default')
  await put(base+'/agents',{client_id:'default',provider:'crabot',role:'持久角色定义',expected_version:defaultAgent.version})
  await stop(server);server=await start(dir,{ADMIN_AGENT_PROVIDER:'mock'})
  const restored=(await request(server,base+'/agents')).agents
  assert.equal(restored.find(a=>a.id==='default').role,'持久角色定义')
  assert.equal(restored.find(a=>a.id==='configured').configuration.values.MODEL_NAME,'edited-model')
  const next=await request(server,base+'/groups/'+group.key+'/messages',{content:'TEST_PLAN:[]'})
  assert.match(JSON.stringify(await history(server,p,next.id)),/Fixture complete/)
  // Old launcher defaults must not override saved model settings, including
  // for business Agents that inherit them rather than owning a configuration.
  await stop(server);server=await start(dir,{ADMIN_AGENT_PROVIDER:'mock',MODEL_PROVIDER:'mock',CRABOT_CONFIG_DEFAULT_KEYS:'MODEL_PROVIDER'})
  const inherited=await request(server,'/v1/admin-agent/configuration')
  assert.equal(inherited.configuration.values.MODEL_PROVIDER,'compatible')
  await put(base+'/agents',{client_id:'inherited',provider:'crabot',role:'inherited model',expected_version:0})
  await request(server,base+'/agents/inherited/start',{})
  const inheritedGroup=await request(server,base+'/groups',{policy:{...policy('inherited'),mode:'chat'}})
  const inheritedRun=await request(server,base+'/groups/'+inheritedGroup.key+'/messages',{content:'TEST_PLAN:[]'})
  assert.match(JSON.stringify(await history(server,p,inheritedRun.id)),/Fixture complete/)
 }finally{await stop(server);await model.close();await rm(dir,{recursive:true,force:true})}
})
