import {readState} from './state-fixture.mjs'
import test from 'node:test'
import {request as httpRequest} from 'node:http'
import assert from 'node:assert/strict'
import {mkdtemp,readFile,writeFile,readdir,rm,mkdir} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {start,stop,request,pause,modelFixture,act,policy,history} from './admin-fixture.mjs'
test('management is loopback-only and ignores obsolete admin tokens',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'crabot-auth-'));let server
  try{
    server=await start(dir,{ADMIN_TOKEN:'',ADMIN_AGENT_PROVIDER:'mock'})
    assert.equal((await fetch(server.url+'/v1/repl')).status,200)
    for(const headers of [{origin:'https://evil.example'},{host:'rebinding.example'},{'sec-fetch-site':'cross-site'}])assert.equal(await new Promise((resolve,reject)=>{const req=httpRequest(server.url+'/v1/repl',{headers},res=>{res.resume();resolve(res.statusCode)});req.on('error',reject);req.end()}),403,JSON.stringify(headers))
    assert.ok(!(await readdir(dir)).includes('admin-token'))
    await stop(server)
    server=await start(dir,{ADMIN_TOKEN:'secret-fixture',ADMIN_AGENT_PROVIDER:'mock'})
    assert.equal((await fetch(server.url+'/v1/repl')).status,200)
    assert.ok((await request(server,'/v1/repl')).projects.length)
  }finally{await stop(server);await rm(dir,{recursive:true,force:true})}
})
test('admin chat persists in the index, paginates, and survives restart',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'crabot-jsonl-'));let server
  try{
    server=await start(dir,{ADMIN_AGENT_PROVIDER:'mock'})
    const p=(await request(server,'/v1/repl')).projects[0].id,base='/v1/admin-agent/'+p+'/sessions'
    const one=await request(server,base,{}),two=await request(server,base,{})
    assert.equal(one.id,two.id)
    for(const content of ['first','second']){
      await request(server,base+'/'+one.id+'/messages',{content})
      for(let i=0;i<100;i++){if((await request(server,base+'/'+one.id)).status!=='running')break;await pause(20)}
    }
    const logs=await request(server,'/v1/repl/'+p+'/chats/admin/history')
    assert.deepEqual(logs.events.filter(e=>e.type==='user').map(e=>e.content),['first','second'])
    assert.equal(logs.files,undefined)
    const page=await request(server,'/v1/repl/'+p+'/chats/admin/history?limit=2')
    const older=await request(server,'/v1/repl/'+p+'/chats/admin/history?before='+page.events[0].seq)
    assert.equal(older.events.at(-1).seq+1,page.events[0].seq)
    assert.ok((await readdir(join(dir,'knowledge',p))).includes('graph.db'))
    assert.ok(!(await readdir(dir)).includes('chats'))
    await stop(server)
    server=await start(dir,{ADMIN_AGENT_PROVIDER:'mock'})
    const restored=await request(server,'/v1/repl/'+p+'/chats/admin/history')
    assert.deepEqual(restored.events,logs.events)
    assert.equal((await fetch(server.url+'/v1/repl/'+p+'/chats/admin/logs')).status,404)
  }finally{await stop(server);await rm(dir,{recursive:true,force:true})}
})
test('group ID owns all rounds without continuation links',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'crabot-group-log-')),model=await modelFixture();let server
  try{
    server=await start(dir,model.env)
    const p=(await request(server,'/v1/repl')).projects[0].id
    const result=await act(server,p,[{name:'agent_start',input:{client_id:'worker',role:'tester',provider:'mock'}},{name:'group_create',input:{name:'Chat',policy:policy('worker')}}])
    const group=result.outputs[1].key
    for(const content of ['hello round one','hello round two']){
      const run=await request(server,`/v1/repl/${p}/groups/${group}/messages`,{content})
      await history(server,p,run.id)
    }
    const log=await request(server,`/v1/repl/${p}/chats/${group}/history`)
    assert.deepEqual(log.events.filter(e=>e.type==='message.created').map(e=>e.content),['hello round one','hello round two'])
    assert.equal(log.files,undefined);assert.equal(log.active_run,null)
  }finally{await stop(server);await model.close();await rm(dir,{recursive:true,force:true})}
})
