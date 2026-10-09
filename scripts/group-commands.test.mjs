import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {start,stop,request,modelFixture,act,policy} from './admin-fixture.mjs'
test('Web commands modify existing groups and tools are visible without execution',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-commands-')),model=await modelFixture();let server
 try{
  server=await start(dir,model.env)
  const p=(await request(server,'/v1/repl')).projects[0].id
  const result=await act(server,p,[{name:'agent_start',input:{client_id:'first',role:'tester',provider:'mock'}},{name:'agent_start',input:{client_id:'second',role:'tester',provider:'mock'}},{name:'group_create',input:{name:'Team',policy:policy('first')}}])
  const key=result.outputs[2].key,path=`/v1/repl/${p}/groups/${key}/commands`
  assert.equal((await request(server,path,{command:'/agents'})).members.length,1)
  await request(server,path,{command:'/add-agent second reviewer'})
  await request(server,path,{command:'/agent second role architect'})
  assert.equal((await request(server,path,{command:'/agents'})).members[1].role,'architect')
  await request(server,path,{command:'/remove-agent second'})
  assert.equal((await request(server,path,{command:'/agents'})).members.length,1)
  const catalog=await request(server,`/v1/repl/${p}/tools`)
  assert.ok(catalog.business.some(t=>t.name==='shell'))
  assert.ok(!catalog.management.some(t=>t.name==='shell'))
  assert.ok(catalog.agents.some(a=>a.id==='first'))
  const settings=`/v1/repl/${p}/tool-config/business/first`
  const put=async(path,body)=>{const response=await fetch(server.url+path,{method:'PUT',headers:{'x-admin-token':server.token,'content-type':'application/json'},body:JSON.stringify(body)});return {status:response.status,value:await response.json()}}
  const initial=await request(server,settings)
  assert.equal(initial.version,0)
  const current=(await request(server,`/v1/repl/${p}/groups`)).find(g=>g.key===key)
  const configuration=`/v1/repl/${p}/groups/${key}/configuration`
  const configured=await put(configuration,{expected_version:current.version,name:'Updated team',policy:{...current.body.policy,mode:'pmo',leader:['first'],rounds:3,instructions:'Review all changes',relay_strategy:'negotiated'}})
  assert.equal(configured.status,200,JSON.stringify(configured.value));assert.equal(configured.value.body.name,'Updated team')
  assert.equal(configured.value.body.policy.mode,'pmo');assert.equal(configured.value.body.policy.rounds,3)
  assert.equal((await put(configuration,{expected_version:current.version,policy:current.body.policy})).status,409)
  await request(server,path,{command:'/group mode discussion'})
  assert.equal((await request(server,`/v1/repl/${p}/groups`)).find(g=>g.key===key).body.policy.mode,'a2a')
  await request(server,path,{command:'/group mode leader'})
  assert.equal((await request(server,`/v1/repl/${p}/groups`)).find(g=>g.key===key).body.policy.mode,'pmo')
  assert.ok(!catalog.business.some(t=>t.name==='skill_read'))
  assert.ok(!catalog.management.some(t=>t.name==='skill_read'))
  const toolPolicy={disabled:['shell'],external:[{name:'check_repo',description:'Check repository',command:'pwd',enabled:true}]}
  assert.equal((await put(settings,{expected_version:0,policy:toolPolicy})).status,200)
  assert.equal((await put(settings,{expected_version:0,policy:toolPolicy})).status,409)
  assert.deepEqual((await request(server,settings)).policy,toolPolicy)
  assert.deepEqual((await request(server,`/v1/repl/${p}/tool-config/business/second`)).policy.external,[])
  const skillPath=`/v1/repl/${p}/skills/business`
  const definition={id:'guide',description:'Test guide',enabled:true,allow_python:false,files:{'SKILL.md':'# Hello'}}
  assert.equal((await put(skillPath,{expected_version:0,definition})).status,200)
  assert.ok((await request(server,skillPath)).some(row=>row.definition.id==='guide'))
  assert.equal((await put(skillPath,{expected_version:1,definition:{...definition,allow_python:true}})).status,400)
  await stop(server);server=await start(dir,{ADMIN_AGENT_PROVIDER:'mock'})
  assert.deepEqual((await request(server,settings)).policy,toolPolicy)
  assert.ok((await request(server,skillPath)).some(row=>row.definition.id==='guide'))
  assert.equal((await fetch(server.url+settings,{method:'PUT',headers:{'content-type':'application/json',origin:'https://evil.example'},body:JSON.stringify({expected_version:1,policy:{disabled:[],external:[]}})})).status,403)
  assert.equal((await put(settings,{expected_version:1,policy:{disabled:[],external:[]}})).status,200)
  assert.deepEqual((await request(server,settings)).policy.external,[])
 }finally{await stop(server);await model.close();await rm(dir,{recursive:true,force:true})}
})
