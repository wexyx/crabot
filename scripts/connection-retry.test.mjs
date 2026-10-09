import test from 'node:test'
import assert from 'node:assert/strict'
import {createServer} from 'node:http'
import {once} from 'node:events'
import {mkdtemp,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {start,stop,request,pause} from './admin-fixture.mjs'
test('failed connections attempt once, deduplicate and allow manual retry/delete',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-retry-'));let node,calls=0
 const peer=createServer((req,res)=>{calls++;res.writeHead(503);res.end('unavailable')})
 peer.listen(0,'127.0.0.1');await once(peer,'listening')
 try{
  node=await start(dir,{ADMIN_AGENT_PROVIDER:'mock'})
  const p=(await request(node,'/v1/repl')).projects[0].id,base=`/v1/repl/${p}/peers`
  const url=`http://127.0.0.1:${peer.address().port}`
  const approval=await request(node,base+'/mount',{url})
  await request(node,`/v1/admin-agent/${p}/approvals/${approval.id}`,{allow:true})
  const waitFailed=async()=>{for(let i=0;i<100;i++){const row=(await request(node,base))[0];if(row.status==='failed')return row;await pause(30)}throw Error('failed state missing')}
  const row=await waitFailed(),first=calls
  await pause(3300);assert.equal(calls,first)
  assert.equal((await request(node,base+'/mount',{url:url+'/'})).name,row.name)
  assert.equal((await request(node,base)).length,1)
  const configure=async body=>{const r=await fetch(node.url+base,{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({name:row.name,direction:'upstream',...body})});assert.equal(r.status,200,await r.text())}
  await configure({disabled:false});await waitFailed();assert.equal(calls,first+1)
  await configure({disabled:true,deleted:true});assert.deepEqual(await request(node,base),[])
  await stop(node);node=await start(dir,{ADMIN_AGENT_PROVIDER:'mock'})
  await pause(350);assert.equal(calls,first+1);assert.deepEqual(await request(node,base),[])
 }finally{await stop(node);await new Promise(r=>peer.close(r));await rm(dir,{recursive:true,force:true})}
})
