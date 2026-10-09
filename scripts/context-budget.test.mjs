import test from 'node:test'
import assert from 'node:assert/strict'
import {createServer} from 'node:http'
import {once} from 'node:events'
import {mkdtemp,rm} from 'node:fs/promises'
import {join} from 'node:path'
import {tmpdir} from 'node:os'
import {start,stop,request,policy,history,pause} from './admin-fixture.mjs'

test('context compaction bounds requests; /new survives restart without deleting group/admin logs',{timeout:60000},async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-context-')),calls=[]
 const model=createServer(async(req,res)=>{
  let raw='';for await(const chunk of req)raw+=chunk
  calls.push(raw)
  const summarizing=JSON.parse(raw).messages.some(m=>typeof m.content==='string'&&m.content.startsWith('COMPACTION TASK:'))
  const text=summarizing?JSON.stringify({goals:['OLD_USER_MARKER'],constraints:[],decisions:[],completed:['OLD_RESULT_'],pending:[],risks:[],references:[]}):calls.length===1?'OLD_RESULT_'+ '中文'.repeat(30000):'Fixture context complete'
  res.writeHead(200,{'content-type':'text/event-stream'})
  res.end('data: '+JSON.stringify({choices:[{delta:{content:text},finish_reason:'stop'}]})+'\n\ndata: [DONE]\n\n')
 })
 model.listen(0,'127.0.0.1');await once(model,'listening')
 const env={ADMIN_AGENT_PROVIDER:'crabot',MODEL_PROVIDER:'compatible',MODEL_API:'chat',MODEL_NAME:'fixture',MODEL_API_KEY:'fixture',MODEL_BASE_URL:`http://127.0.0.1:${model.address().port}/v1`,CONTEXT_MAX_TOKENS:'65536',CONTEXT_STRATEGY:'extractive'}
 let server
 try{
  server=await start(dir,env)
  const p=(await request(server,'/v1/repl')).projects[0].id,base=`/v1/repl/${p}`
  const group=await request(server,base+'/groups',{policy:{...policy('default'),mode:'chat'}})
  const send=async content=>{
   const run=await request(server,`${base}/groups/${group.key}/messages`,{content})
   return history(server,p,run.id)
  }
  await send('OLD_USER_MARKER')
  const second=await send('LATEST_REQUEST_KEEP')
  assert.ok(calls.length>=2,JSON.stringify(second))
  assert.match(calls.at(-1),/LATEST_REQUEST_KEEP/)
  assert.ok(Buffer.byteLength(calls.at(-1))<=65536-4096)
  assert.match(JSON.stringify(second),/Context compacted/)
  const other=await request(server,base+'/groups',{policy:{...policy('default'),mode:'chat'}})
  const otherRun=await request(server,`${base}/groups/${other.key}/messages`,{content:'OTHER_GROUP_MARKER'})
  await history(server,p,otherRun.id)
  await request(server,`${base}/groups/${group.key}/commands`,{command:'/new'})
  await stop(server);server=await start(dir,env)
  await send('NEW_REQUEST_KEEP')
  assert.doesNotMatch(calls.at(-1),/OLD_USER_MARKER|OLD_RESULT_|LATEST_REQUEST_KEEP|OTHER_GROUP_MARKER/)
  const logs=await request(server,`${base}/chats/${group.key}/history?limit=100`)
  assert.match(JSON.stringify(logs),/OLD_USER_MARKER/)
  const otherNext=await request(server,`${base}/groups/${other.key}/messages`,{content:'OTHER_CONTINUE'})
  await history(server,p,otherNext.id)
  assert.match(calls.at(-1),/OTHER_GROUP_MARKER/)
  const adminBase=`/v1/admin-agent/${p}/sessions`
  const session=await request(server,adminBase,{})
  const adminSend=async content=>{
   await request(server,`${adminBase}/${session.id}/messages`,{content})
   for(let i=0;i<200;i++){
    const row=await request(server,`${adminBase}/${session.id}`)
    if(row.status!=='running'){assert.equal(row.status,'completed',JSON.stringify(row));return row}
    await pause(25)
   }
   throw Error('admin timeout')
  }
  await adminSend('OLD_ADMIN_MARKER')
  await request(server,`${adminBase}/${session.id}/new`,{})
  const adminHistory=await adminSend('NEW_ADMIN_MARKER')
  assert.doesNotMatch(calls.at(-1),/OLD_ADMIN_MARKER/)
  assert.match(JSON.stringify(adminHistory),/OLD_ADMIN_MARKER/)
 }finally{await stop(server);await new Promise(r=>model.close(r));await rm(dir,{recursive:true,force:true})}
})
