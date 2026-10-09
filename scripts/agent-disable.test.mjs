import test from 'node:test'
import assert from 'node:assert/strict'
import {createServer} from 'node:http'
import {once} from 'node:events'
import {mkdtemp,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {start,stop,request,history,policy} from './admin-fixture.mjs'
test('disabling an Agent rejects new work without interrupting its running task',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-disable-'));let node,response,entered
 const started=new Promise(r=>entered=r)
 const model=createServer(async(req,res)=>{for await(const chunk of req){}response=res;entered()})
 model.listen(0,'127.0.0.1');await once(model,'listening')
 try{
  node=await start(dir,{ADMIN_AGENT_PROVIDER:'mock'})
  const p=(await request(node,'/v1/repl')).projects[0].id,base=`/v1/repl/${p}`
  const saved=await fetch(node.url+base+'/agents',{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({client_id:'worker',provider:'crabot',role:'fixture',expected_version:0,configuration:{MODEL_PROVIDER:'compatible',MODEL_NAME:'fixture',MODEL_API:'chat',MODEL_BASE_URL:`http://127.0.0.1:${model.address().port}/v1`,MODEL_API_KEY:'fixture'}})})
  assert.equal(saved.status,200)
  await request(node,base+'/agents/worker/start',{})
  const group=await request(node,base+'/groups',{policy:{...policy('worker'),mode:'chat'}})
  const run=await request(node,base+'/groups/'+group.key+'/messages',{content:'finish this task'})
  await Promise.race([started,new Promise((_,reject)=>setTimeout(()=>reject(Error('model never started')),4000))])
  const approval=await request(node,base+'/agents/worker/stop',{})
  await request(node,`/v1/admin-agent/${p}/approvals/${approval.id}`,{allow:true})
  assert.equal((await request(node,base+'/agents/candidates')).agents.find(a=>a.id==='worker').online,false)
  response.writeHead(200,{'content-type':'text/event-stream'})
  response.end('data: '+JSON.stringify({choices:[{delta:{content:'Finished after disable'},finish_reason:'stop'}]})+'\n\ndata: [DONE]\n\n')
  const events=await history(node,p,run.id)
  assert.ok(events.some(e=>e.type==='agent.message'&&e.payload.content.includes('Finished after disable')))
  assert.ok(!events.some(e=>e.type==='agent.error'||e.type==='task.interrupted'))
  const second=await request(node,base+'/groups/'+group.key+'/messages',{content:'must reject'})
  const rejected=await history(node,p,second.id)
  assert.ok(rejected.some(e=>e.type==='agent.error'))
 }finally{response?.end();await stop(node);model.closeAllConnections();await new Promise(r=>model.close(r));await rm(dir,{recursive:true,force:true})}
})
