import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {start,stop,request,policy} from './admin-fixture.mjs'
test('chat rename and soft deletion persist without deleting Agents',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-chat-menu-'));let server
 try{
  server=await start(dir,{ADMIN_AGENT_PROVIDER:'mock'})
  const p=(await request(server,'/v1/repl')).projects[0].id
  const group=await request(server,`/v1/repl/${p}/groups`,{name:'Before',policy:{...policy('default'),mode:'chat'}})
  const url=`/v1/repl/${p}/groups/${group.key}`
  const update=async(method,path,body,status=200)=>{
   const response=await fetch(server.url+path,{method,headers:{'content-type':'application/json'},body:JSON.stringify(body)})
   assert.equal(response.status,status,await response.clone().text());return response.json()
  }
  const renamed=await update('PUT',url+'/name',{name:'After',expected_version:group.version})
  assert.equal(renamed.body.name,'After');assert.equal(renamed.body.auto_name,false)
  await update('DELETE',url,{expected_version:group.version},409)
  const removed=await update('DELETE',url,{expected_version:renamed.version})
  assert.equal(removed.history_retained,true)
  assert.ok(!(await request(server,'/v1/repl')).collaboration_projects.some(g=>g.key===group.key))
  await stop(server);server=await start(dir,{ADMIN_AGENT_PROVIDER:'mock'})
  assert.ok(!(await request(server,'/v1/repl')).collaboration_projects.some(g=>g.key===group.key))
  assert.ok((await request(server,`/v1/repl/${p}/agents/candidates`)).agents.some(a=>a.id==='default'))
 }finally{await stop(server);await rm(dir,{recursive:true,force:true})}
})
