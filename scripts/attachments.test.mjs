import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {start,stop,request,pause} from './admin-fixture.mjs'
test('uploads use local guarded URLs, accept messages, and preserve indexed references',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-attachment-test-'));let server
 try{
  server=await start(dir,{ADMIN_AGENT_PROVIDER:'mock'})
  const upload=await fetch(server.url+'/v1/attachments?name=hello.txt',{method:'POST',headers:{'content-type':'application/octet-stream'},body:'ATTACHMENT_CONTENT'})
  assert.equal(upload.status,200);const file=await upload.json()
  assert.equal(await (await fetch(server.url+file.attachment.url)).text(),'ATTACHMENT_CONTENT')
  assert.equal((await fetch(server.url+file.attachment.url,{headers:{origin:'https://evil.example'}})).status,403)
  assert.equal((await fetch(server.url+file.attachment.url+'?preview=true')).status,415)
  const png=Buffer.from('89504e470d0a1a0a','hex')
  const image=await (await fetch(server.url+'/v1/attachments?name=shot.png',{method:'POST',body:png})).json()
  const preview=await fetch(server.url+image.attachment.url+'?preview=true')
  assert.equal(preview.headers.get('content-type'),'image/png')
  assert.deepEqual(Buffer.from(await preview.arrayBuffer()),png)
  const invalid=await fetch(server.url+'/v1/attachments?name=..%2Fsecret',{method:'POST',body:'x'});assert.ok(!invalid.ok)
  const p=(await request(server,'/v1/repl')).projects[0].id
  const base=`/v1/admin-agent/${p}/sessions`,session=await request(server,base,{})
  const content='Analyze this file.\n'+file.reference
  await request(server,`${base}/${session.id}/messages`,{content})
  let row
  for(let i=0;i<200;i++){row=await request(server,`${base}/${session.id}`);if(row.status!=='running')break;await pause(20)}
  assert.equal(row.status,'completed',JSON.stringify(row))
  assert.equal(row.events.find(e=>e.type==='user').content,content)
  assert.ok(row.events.some(e=>e.type==='completed'&&e.text.includes('Mock Agent')))
  const logs=await request(server,`/v1/repl/${p}/chats/admin/history`)
  assert.ok(logs.events.some(e=>e.type==="user"&&e.content===content))
 }finally{await stop(server);await rm(dir,{recursive:true,force:true})}
})
