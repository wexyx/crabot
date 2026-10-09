import {readState} from './state-fixture.mjs'
// Real process + model tool roundtrip + HTTP + durable files. No external model calls.
import test from 'node:test'
import {randomUUID,randomBytes} from 'node:crypto'
import assert from 'node:assert/strict'
import {mkdtemp,readFile,writeFile,rm,readdir} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {modelFixture,start,stop,request,act,policy,history,pause} from './admin-fixture.mjs'
test('AdminAgent tools persist groups, conversations and enrollment across restart',{timeout:30000},async()=>{
  const dir=await mkdtemp(join(tmpdir(),'crabot-admin-files-')),model=await modelFixture();let server
  try{
    server=await start(dir,model.env)
    const token=server.token,profile=await request(server,'/v1/agent')
    const project=(await request(server,'/v1/repl')).projects[0].id
    const result=await act(server,project,[
      {name:'agent_start',input:{client_id:'worker',role:'tester',provider:'mock'}},
      {name:'group_create',input:{name:'Persistent group',policy:policy('worker')}}
    ])
    assert.ok(result.outputs.every(r=>!r.error),JSON.stringify(result.outputs))
    const group=result.outputs[1],registration={node_id:randomUUID(),registration_secret:randomBytes(32).toString('hex'),name:'fixture'}
    const run=await request(server,`/v1/repl/${project}/groups/${group.key}/messages`,{content:'Original task'})
    await history(server,project,run.id)
    const registrations=await Promise.all([1,2].map(()=>fetch(server.url+'/v1/client/register',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify(registration)})))
    assert.deepEqual(registrations.map(r=>r.status).sort(),[201,201])
    const keys=await registrations.find(r=>r.status===201).json()
    await stop(server);server=await start(dir,model.env)
    assert.equal(server.token,token);assert.equal((await request(server,'/v1/agent')).id,profile.id)
    assert.deepEqual((await request(server,`/v1/admin-agent/${project}/sessions/${result.session.id}`)).events,result.session.events)
    assert.equal((await request(server,`/v1/repl/${project}/groups`))[0].key,group.key)
    const next=await request(server,`/v1/repl/${project}/groups/${group.key}/messages`,{content:'Continue task',previous_session_id:run.id})
    await history(server,project,next.id)
    const connection=await fetch(server.url+'/v1/client/connect',{headers:{'x-agent-ak':keys.ak,'x-agent-sk':keys.sk}})
    assert.equal(connection.status,200);await connection.body.cancel()
    assert.equal((await fetch(server.url+'/v1/client/connect',{headers:{'x-agent-ak':keys.ak,'x-agent-sk':'wrong'}})).status,401)
    assert.equal((await fetch(server.url+'/v1/crabot/control',{method:'POST'})).status,404)
    await stop(server)
    const snapshot=await readState(dir)
    assert.equal(snapshot.collections.history,undefined)
    assert.equal(snapshot.collections.runs[next.id].prompt,"Continue task")
    assert.ok(snapshot.collections.credentials[keys.ak])
    assert.ok(!(await readdir(dir)).some(p=>/sqlite|\.db$/.test(p)))
  }finally{await stop(server);await model.close();await rm(dir,{recursive:true,force:true})}
})
test('interrupted business CLI context survives restart through group conversation',{timeout:30000},async()=>{
  const dir=await mkdtemp(join(tmpdir(),'crabot-admin-resume-')),model=await modelFixture();let server
  const cli=join(dir,'fake-codex')
  await writeFile(cli,`#!${process.execPath}
const prompt=process.argv.at(-1);
console.log(JSON.stringify({type:'item.completed',item:{id:'partial',type:'agent_message',text:'checkpoint: inspected existing files'}}));
if(!prompt.includes('REVISED_REQUEST'))setTimeout(()=>{},15000);
else {if(!prompt.includes('ORIGINAL_REQUEST')||!prompt.includes('checkpoint: inspected existing files'))throw Error('missing prior context');console.log(JSON.stringify({type:'turn.completed'}));}
`,{mode:0o700})
  const env={...model.env,CODEX_BIN:cli,AGENT_WORKDIR:dir}
  try{
    server=await start(join(dir,'data'),env)
    const project=(await request(server,'/v1/repl')).projects[0].id
    const result=await act(server,project,[{name:'agent_start',input:{client_id:'coder',role:'developer',provider:'codex'}},{name:'group_create',input:{name:'Code',policy:policy('coder')}}])
    const group=result.outputs[1]
    const first=await request(server,`/v1/repl/${project}/groups/${group.key}/messages`,{content:'ORIGINAL_REQUEST'})
    await history(server,project,first.id,rows=>rows.some(r=>r.type==='agent.delta'&&r.payload.content.includes('checkpoint')))
    await request(server,`/v1/sessions/${first.id}/interrupt`,{})
    await pause(300)
    await stop(server);server=await start(join(dir,'data'),env)
    const next=await request(server,`/v1/repl/${project}/groups/${group.key}/messages`,{content:'REVISED_REQUEST',previous_session_id:first.id})
    await history(server,project,next.id)
    await stop(server)
    const snapshot=await readState(join(dir,'data'))
    assert.equal(snapshot.collections.runs[next.id].prompt,'REVISED_REQUEST')
    server=await start(join(dir,'data'),env)
    const log=JSON.stringify(await request(server,`/v1/repl/${project}/chats/${group.key}/history`))
    for(const term of ['ORIGINAL_REQUEST','checkpoint: inspected existing files','REVISED_REQUEST'])assert.ok(log.includes(term))

  }finally{await stop(server);await model.close();await rm(dir,{recursive:true,force:true})}
})
