import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,mkdir,writeFile,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {start,stop,request,modelFixture,policy,history} from './admin-fixture.mjs'

test('project workspace persists and sets shell working directory',{timeout:30000},async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-project-workspace-')),model=await modelFixture()
 let server
 try{
  for(const name of ['alpha','beta']){
   await mkdir(join(dir,name));await writeFile(join(dir,name,'marker.txt'),name+'-workspace-marker')
  }
  server=await start(join(dir,'data'),{...model.env,AGENT_WORKDIR:dir,AGENT_OUTSIDE_ACCESS:'deny'})
  const p=(await request(server,'/v1/repl')).projects[0].id,base='/v1/repl/'+p
  const permission=await request(server,base+'/agents/default/permissions')
  const changed=await fetch(server.url+base+'/agents/default/permissions',{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({mode:'full',expected_version:permission.version,confirm_full_access:true})});assert.equal(changed.status,200)
  const make=workspace=>request(server,base+'/groups',{name:'Workspace',policy:{...policy('default'),mode:'chat'},workspace})
  for(const name of ['alpha','beta']){
   const group=await make({workdir:join(dir,name),outside_access:'deny'})
   assert.equal(group.body.workspace.workdir,join(dir,name))
   const run=await request(server,base+'/groups/'+group.key+'/messages',{content:'TEST_PLAN:'+JSON.stringify([{name:'shell',input:{command:'cat marker.txt'}}])})
   const events=await history(server,p,run.id)
   assert.ok(JSON.stringify(events).includes(name+'-workspace-marker'),JSON.stringify(events))
   const response=await fetch(server.url+base+'/groups/'+group.key+'/configuration',{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({name:'Updated workspace',policy:group.body.policy,expected_version:group.version,workspace:{workdir:join(dir,'beta'),outside_access:'deny'}})})
   assert.equal(response.status,200,await response.clone().text())
   assert.equal((await response.json()).body.workspace.workdir,join(dir,'beta'))
  }
  for(const workspace of [{workdir:'.'},{workdir:join(dir,'missing')},{outside_access:'allow'},{outside_access:'ask'}]){
   const response=await fetch(server.url+base+'/groups',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({policy:policy('default'),workspace})})
   assert.equal(response.status,400,JSON.stringify(workspace))
  }
 }finally{await stop(server);await model.close();await rm(dir,{recursive:true,force:true})}
})
