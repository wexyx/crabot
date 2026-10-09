import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {start,stop,request,history} from './admin-fixture.mjs'
import {conversationView} from '../apps/web/src/conversation-view.js'

// A two-member group, so a message has somewhere to go other than "everybody".
async function fixture(name,run){
  const dir=await mkdtemp(join(tmpdir(),`crabot-at-${name}-`));let server
  try{
    server=await start(dir,{ADMIN_AGENT_PROVIDER:'mock'})
    const p=(await request(server,'/v1/repl')).projects[0].id,base='/v1/repl/'+p
    const response=await fetch(server.url+base+'/agents',{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({client_id:'second',role:'reviewer',provider:'mock',expected_version:0})})
    assert.equal(response.status,200,await response.clone().text())
    await request(server,base+'/agents/second/start',{})
    const group=await request(server,base+'/groups',{policy:{mode:'a2a',members:[{path:['default'],role:'writer'},{path:['second'],role:'reviewer'}],rounds:1,leader:null,instructions:''}})
    await run({server,p,base,group})
  }finally{await stop(server);await rm(dir,{recursive:true,force:true})}
}

test('@name dispatches only to the Agent it addresses',{timeout:30000},async()=>{
  await fixture('single',async({server,p,base,group})=>{
    const run=await request(server,base+'/groups/'+group.key+'/messages',{content:'@second 看一下这个错误'})
    await history(server,p,run.id)
    const logs=await request(server,base+'/chats/'+group.key+'/history')
    const replies=conversationView(logs.events).filter(r=>r.type==='assistant')
    // One member was addressed, so only it answers. Unaddressed would be two.
    assert.deepEqual(replies.map(r=>r.agent),['second'],JSON.stringify(replies))
    // The addressed Agent reads the request itself, not a transcription of it.
    const prompt=logs.events.find(e=>e.type==='message.created')
    assert.equal(prompt.content,'@second 看一下这个错误',JSON.stringify(prompt))
  })
})

test('a mention with no request is refused rather than sent empty',{timeout:30000},async()=>{
  await fixture('bare',async({server,base,group})=>{
    const response=await fetch(server.url+base+'/groups/'+group.key+'/messages',{method:'POST',headers:{'x-admin-token':server.token,'content-type':'application/json'},body:JSON.stringify({content:'@second'})})
    assert.equal(response.status,400)
    assert.match((await response.json()).error,/mention needs a request/)
  })
})

test('an @name that is not a member stays prose and the group still answers',{timeout:30000},async()=>{
  await fixture('unknown',async({server,p,base,group})=>{
    const run=await request(server,base+'/groups/'+group.key+'/messages',{content:'ping @nobody or mail@example.com'})
    await history(server,p,run.id)
    const logs=await request(server,base+'/chats/'+group.key+'/history')
    const replies=conversationView(logs.events).filter(r=>r.type==='assistant')
    assert.deepEqual(replies.map(r=>r.agent),['default','second'],JSON.stringify(replies))
  })
})

test('the next unaddressed turn reaches the whole group again',{timeout:30000},async()=>{
  await fixture('then-broad',async({server,p,base,group})=>{
    const addressed=await request(server,base+'/groups/'+group.key+'/messages',{content:'@second 先看你'})
    await history(server,p,addressed.id)
    const next=await request(server,base+'/groups/'+group.key+'/messages',{content:'大家都看一下'})
    await history(server,p,next.id)
    const logs=await request(server,base+'/chats/'+group.key+'/history')
    const replies=conversationView(logs.events).filter(r=>r.type==='assistant')
    // One addressed reply plus a full-group reply: narrowing must not be permanent.
    assert.deepEqual(replies.map(r=>r.agent),['second','default','second'],JSON.stringify(replies))
    const groups=(await request(server,'/v1/repl')).collaboration_projects
    const saved=groups.find(g=>g.key===group.key)
    assert.equal(saved.body.policy.members.length,2,JSON.stringify(saved.body.policy))
  })
})
