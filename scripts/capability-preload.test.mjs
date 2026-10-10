import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {modelFixture,start,stop,request,act,policy,history} from './admin-fixture.mjs'

test('management and project first requests contain enabled builtins and emit tool arguments',{timeout:30000},async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-preload-')),requests=[]
 const model=await modelFixture(body=>requests.push(body));let server
 try{
  server=await start(join(dir,'data'),model.env)
  const project=(await request(server,'/v1/repl')).projects[0].id
  const result=await act(server,project,[{name:'skill',input:{action:'list'}}])
  assert.ok(result.outputs[0].some(s=>s.name==='skill-creator'))
  const management=requests[0]
  const group=await request(server,`/v1/repl/${project}/groups`,{policy:{...policy('default'),mode:'chat'}})
  const before=requests.length
  const run=await request(server,`/v1/repl/${project}/groups/${group.key}/messages`,{content:'TEST_PLAN:'+JSON.stringify([{name:'skill',input:{action:'list'}}])})
  const rows=await history(server,project,run.id)
  const business=requests[before]
  for(const body of [management,business]){
   const names=body.tools.map(t=>t.function.name)
   for(const name of ['find','skill','compact','doc'])assert.ok(names.includes(name),name)
   const prompt=body.messages.find(m=>m.role==='user').content
   assert.match(prompt,/Skill Creator/)
   assert.match(prompt,/work\/memory\.md/)
   const line=prompt.split('\n').find(l=>l.startsWith('Skills: '))
   const skills=JSON.parse(line.slice(8))
   assert.ok(skills.some(s=>s.id==='memory'&&s.instructions))
   if(body===business)assert.ok(skills.some(s=>s.id==='browser-automation'&&s.directory))
   assert.ok(skills.some(s=>s.id==='skill-creator'&&s.instructions))
  }
  assert.ok(business.tools.some(t=>t.function.name==='shell'))
  const event=rows.find(r=>r.type==='agent.tool.started')
  const content=JSON.parse(event.payload.content)
  assert.equal(content.name,'skill')
  assert.deepEqual(content.arguments,{action:'list'})
 }finally{await stop(server);await model.close();await rm(dir,{recursive:true,force:true})}
})
