import {readState} from './state-fixture.mjs'
import test from 'node:test'
import assert from 'node:assert/strict'
import {createServer} from 'node:http'
import {once} from 'node:events'
import {mkdtemp,readFile,rm} from 'node:fs/promises'
import {join} from 'node:path'
import {tmpdir} from 'node:os'
import {start,stop,request} from './admin-fixture.mjs'
import {createSseDecoder} from '../apps/web/src/agent-api.js'

test('live SSE is prompt, fragments are batched, final state is durable, interruption flushes progress',async t=>{
  const dir=await mkdtemp(join(tmpdir(),'crabot-streaming-'))
  let calls=0,server
  const model=createServer(async(req,res)=>{
    let text='';for await(const part of req)text+=part
    const body=JSON.parse(text);assert.ok(!body.messages[1].content.includes('First read management-guide'))
    calls++;res.writeHead(200,{'content-type':'text/event-stream'})
    let index=0
    const timer=setInterval(()=>{
      if(index++<200)res.write('data: '+JSON.stringify({choices:[{delta:{content:'字'},finish_reason:null}]})+'\n\n')
      else {clearInterval(timer);res.end('data: '+JSON.stringify({choices:[{delta:{},finish_reason:'stop'}]})+'\n\n')}
    },4)
    res.on('close',()=>clearInterval(timer))
  })
  model.listen(0,'127.0.0.1');await once(model,'listening')
  try{
    server=await start(dir,{ADMIN_AGENT_PROVIDER:'crabot',MODEL_PROVIDER:'compatible',MODEL_API:'chat',MODEL_NAME:'fixture',MODEL_API_KEY:'fixture',MODEL_BASE_URL:`http://127.0.0.1:${model.address().port}`})
    const info=await request(server,'/v1/repl'),project=info.projects[0].id
    const base=`/v1/admin-agent/${project}/sessions`
    async function run(interrupt){
      const session=await request(server,base,{})
      const before=(await request(server,base+'/'+session.id)).events.at(-1)?.seq||0
      const controller=new AbortController(),events=[]
      const connectAt=performance.now()
      const response=await fetch(server.url+base+'/'+session.id+'/events?after='+before,{headers:{'x-admin-token':server.token},signal:controller.signal})
      assert.ok(performance.now()-connectAt<1000,'empty-session SSE must flush headers immediately')
      let resolveDone,first=0,interruptTask
      const done=new Promise(resolve=>resolveDone=resolve)
      const began=performance.now()
      const push=createSseDecoder(event=>{
        events.push(event)
        if(event.type==='text_delta'&&!first){first=performance.now()-began;if(interrupt)interruptTask=request(server,base+'/'+session.id+'/interrupt',{})}
        if(event.type==='completed'||event.type==='failed')resolveDone()
      })
      const reader=response.body.getReader()
      const consume=(async()=>{try{while(true){const {value,done}=await reader.read();if(done)break;push(value)}}catch(e){if(e.name!=='AbortError')throw e}})()
      await request(server,base+'/'+session.id+'/messages',{content:'你好'})
      let timeout
      try{await Promise.race([done,new Promise((_,reject)=>{timeout=setTimeout(()=>reject(new Error('stream did not finish')),5000)})])}
      finally{clearTimeout(timeout);controller.abort();await consume}
      if(interruptTask)await interruptTask
      const disk=(await readState(dir)).collections.management_sessions[session.id]
      assert.equal(disk.status,interrupt?'failed':'completed')
      assert.equal(disk.events,undefined)
      const saved=await request(server,`/v1/repl/${project}/chats/admin/history?after=${before}`)
      assert.deepEqual(saved.events,events)
      const fragments=events.filter(e=>e.type==='text_delta')
      assert.ok(first<600,'first output should arrive before the whole answer')
      if(!interrupt){assert.equal(fragments.map(e=>e.text).join(''),'字'.repeat(200));assert.ok(fragments.length<60);assert.equal(calls,1)}
      else assert.ok(fragments.length>0&&fragments.map(e=>e.text).join('').length<200)
      return {first_ms:Math.round(first),text_events:fragments.length,session:session.id}
    }
    const completed=await run(false),interrupted=await run(true)
    t.diagnostic(JSON.stringify({raw_chunks:200,completed,interrupted,model_api_calls:calls}))
    await stop(server);server=await start(dir,{ADMIN_AGENT_PROVIDER:'mock'})
    const restored=await request(server,base+'/'+completed.session)
    assert.equal(completed.session,interrupted.session);assert.equal(restored.status,'failed');assert.ok(restored.events.some(e=>e.type==='completed'&&e.text==='字'.repeat(200)))
  }finally{await stop(server);model.closeAllConnections();await new Promise(resolve=>model.close(resolve));await rm(dir,{recursive:true,force:true})}
})
