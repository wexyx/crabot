import test from 'node:test'
import assert from 'node:assert/strict'
import {createServer} from 'node:http'
import {once} from 'node:events'
import {mkdtemp,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {start,stop,request,pause,policy,modelFixture,act,history} from './admin-fixture.mjs'

test('Agent imports into the shared library visible to Web and other project Agents',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-agent-doc-'));let server;const received=[];const model=await modelFixture(body=>received.push(body))
 try{
  server=await start(dir,model.env);const p=(await request(server,'/v1/repl')).projects[0].id
  const result=await act(server,p,[{name:'doc',input:{action:'import',path:'docs/knowledge-and-tasks.md'}},{name:'find',input:{query:'知识库'}}])
  assert.ok(result.outputs[0].id,JSON.stringify(result.outputs))
  assert.ok(result.outputs[1].doc.hits.some(d=>d.id===result.outputs[0].id))
  const id=result.outputs[0].id
  assert.ok((await request(server,'/v1/documents',{action:'search',query:'知识库'})).hits.some(d=>d.id===id))
  for(let n=0;n<2;n++){
   received.length=0
   const group=await request(server,`/v1/repl/${p}/groups`,{name:`Reader ${n}`,policy:{...policy('default'),mode:'chat'}})
   const run=await request(server,`/v1/repl/${p}/groups/${group.key}/messages`,{content:'Read the shared document. TEST_PLAN:'+JSON.stringify([{name:'find',input:{target:'doc',id}}])})
   const events=await history(server,p,run.id)
   assert.ok(!events.some(e=>e.type==='agent.error'),JSON.stringify(events))
   const toolResults=received.flatMap(body=>body.messages.filter(m=>m.role==='tool').map(m=>{try{return JSON.parse(m.content)}catch{return null}}))
   assert.ok(toolResults.some(r=>r?.doc?.id===id&&r.doc.chunks?.some(c=>c.text.includes('知识库'))),JSON.stringify(toolResults))
  }
  await stop(server);server=await start(dir,model.env)
  assert.equal((await request(server,'/v1/documents',{action:'read',id})).id,id)
 }finally{await stop(server);await model.close();await rm(dir,{recursive:true,force:true})}
})

test('instance document HTTP CRUD and import are version checked',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-documents-'));let server
 try{
  server=await start(dir,{ADMIN_AGENT_PROVIDER:'mock'})
  const p=(await request(server,'/v1/repl')).projects[0].id
  const base='/v1/documents'
  const row=await request(server,base,{action:'save',title:'Architecture',content:'Rust knowledge graph 文档',source:'https://example.com/architecture'})
  assert.equal(row.version,1)
  assert.ok(row.created_at>0)
  assert.equal((await request(server,base,{action:'search',query:'文档'})).hits[0].id,row.id)
  assert.equal((await request(server,base,{action:'read',id:row.id})).chunks[0].text,'Rust knowledge graph 文档')
  const denied=await fetch(server.url+base,{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({action:'import',url:'http://127.0.0.1/private'})})
  assert.ok(!denied.ok);assert.match(await denied.text(),/private|reserved/)
  const updated=await request(server,base,{action:'save',id:row.id,expected_version:1,title:'Edited',content:'new text'})
  assert.equal(updated.version,2)
  assert.equal(updated.created_at,row.created_at)
  const conflict=await fetch(server.url+base,{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({action:'delete',id:row.id,expected_version:1})})
  assert.ok(!conflict.ok)
  const uploaded=await fetch(server.url+base+'/upload?name=hello.md',{method:'POST',headers:{'content-type':'application/octet-stream'},body:'# Hello\nKnowledge upload'})
  assert.equal(uploaded.status,200)
  const upload=await uploaded.json()
  const firstPage=await request(server,base,{action:'search',limit:1})
  assert.equal(firstPage.total,2)
  assert.equal(firstPage.hits[0].id,upload.id)
  assert.equal(firstPage.next_offset,1)
  const secondPage=await request(server,base,{action:'search',offset:1,limit:1})
  assert.equal(secondPage.hits[0].id,row.id)
  assert.equal(secondPage.truncated,false)
  await request(server,base,{action:'delete',id:row.id,expected_version:2})
  assert.equal((await request(server,base,{action:'search',query:'Edited'})).hits.length,0)
 }finally{await stop(server);await rm(dir,{recursive:true,force:true})}
})

test('management and project accept steering during a model call, keep one task and persist guidance',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-steering-'));let server,release,entered,first=true
 const calls=[]
 const model=createServer(async(req,res)=>{
  let raw='';for await(const chunk of req)raw+=chunk
  const body=JSON.parse(raw);calls.push(body)
  if(first){first=false;entered?.();await new Promise(r=>release=r)}
  const guided=body.messages.some(m=>typeof m.content==='string'&&m.content.includes('GUIDANCE: switch to blue'))
  res.writeHead(200,{'content-type':'text/event-stream'});res.end('data: '+JSON.stringify({choices:[{delta:{content:guided?'blue result':'original result'},finish_reason:'stop'}]})+'\n\ndata: [DONE]\n\n')
 })
 model.listen(0,'127.0.0.1');await once(model,'listening')
 try{
  server=await start(dir,{ADMIN_AGENT_PROVIDER:'crabot',MODEL_PROVIDER:'compatible',MODEL_API:'chat',MODEL_BASE_URL:`http://127.0.0.1:${model.address().port}/v1`,MODEL_NAME:'fixture',MODEL_API_KEY:'fixture'})
  const p=(await request(server,'/v1/repl')).projects[0].id
  for(const management of [true,false]){
   calls.length=0;first=true;let signal;const reached=new Promise(r=>signal=r);entered=signal
   let base,id
   if(management){const session=await request(server,`/v1/admin-agent/${p}/sessions`,{});base=`/v1/admin-agent/${p}/sessions/${session.id}/messages`}
   else {const group=await request(server,`/v1/repl/${p}/groups`,{policy:{...policy('default'),mode:'chat'}});base=`/v1/repl/${p}/groups/${group.key}/messages`}
   const run=await request(server,base,{content:'Start original task'});id=run.id||run.session_id
   await reached
   const guide=await request(server,base,{content:'GUIDANCE: switch to blue'})
   assert.equal(guide.status,'steering_accepted');assert.equal(guide.id||guide.session_id,id)
   release()
   let finished=false
   for(let n=0;n<300;n++){
    const result=management?await request(server,`/v1/admin-agent/${p}/sessions/${id}`):await request(server,`/v1/repl/${p}/sessions/${id}`)
    const rows=management?result.events:result
    if(management?result.status!=='running':rows.some(r=>r.type==='agent.done')){
     const text=JSON.stringify(rows)
     assert.match(text,/GUIDANCE: switch to blue/);assert.match(text,/blue result/)
     assert.ok(!rows.some(r=>r.type==='failed'||r.type==='agent.error'),text)
     finished=true;break
    }
    await pause(25)
   }
   assert.ok(finished,'steered task completed');assert.ok(calls.length>=2)
  }
 }finally{release?.();await stop(server);model.closeAllConnections();await new Promise(r=>model.close(r));await rm(dir,{recursive:true,force:true})}
})
