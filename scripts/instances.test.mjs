import {readState} from './state-fixture.mjs'
import test from 'node:test'
import assert from 'node:assert/strict'
import {spawn} from 'node:child_process'
import {once} from 'node:events'
import {mkdtemp,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join,resolve} from 'node:path'
import {modelFixture,pause} from './admin-fixture.mjs'
test('CLI aliases isolate data, reject duplicate process, and allocate separate Web ports',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-instances-')),model=await modelFixture(),children=[]
 function launch(name,extra={}){
   const child=spawn(resolve('target/debug/agent-node'),['--cli','--workdir',dir,'--name',name,'--web-port','0'],{cwd:dir,env:{HOME:dir,PATH:process.env.PATH,...model.env,...extra},stdio:['pipe','pipe','pipe']})
   const item={child,output:'',name};children.push(item)
   child.stdout.on('data',b=>item.output+=b);child.stderr.on('data',b=>item.output+=b);return item
 }
 async function wait(item,text){for(let i=0;i<200;i++){if(item.output.includes(text))return;if(item.child.exitCode!==null)throw Error(item.output);await pause(20)}throw Error(item.output)}
 async function logs(name){
   const root=join(dir,'.crabot_'+name),state=await readState(root),p=Object.keys(state.collections.projects)[0]
   const address=state.collections.instance_settings.web.address
   const response=await fetch(`http://${address}/v1/repl/${p}/chats/admin/history`)
   assert.equal(response.status,200)
   return (await response.json()).events
 }
 async function web(item,input){
   item.output='';item.child.stdin.write('启动网页 TEST_PLAN:'+JSON.stringify([{name:'server_start',input}])+'\n')
   await wait(item,'Fixture complete\nadmin> ')
   return JSON.parse((await logs(item.name)).filter(e=>e.type==='tool_finished').at(-1).output)
 }
 try{
   const a=launch('alpha'),b=launch('beta');await Promise.all([wait(a,'group>'),wait(b,'group>')])
   a.child.stdin.write('/manage\n');b.child.stdin.write('/manage\n');await Promise.all([wait(a,'admin>'),wait(b,'admin>')])
   const duplicate=launch('alpha',{ADMIN_AGENT_PROVIDER:'unknown'})
   await once(duplicate.child,'exit');assert.equal(duplicate.child.exitCode,2);assert.match(duplicate.output,/already in use.*pid=/);assert.doesNotMatch(duplicate.output,/AdminAgent configuration/)
   const first=await web(a,{});assert.ok(first.url,JSON.stringify(first))
   const rejected=await web(b,{port:Number(new URL(first.url).port)});assert.match(JSON.stringify(rejected),/different port|Address already in use/)
   const second=await web(b,{port:0});assert.ok(second.url);assert.notEqual(second.url,first.url)
   assert.equal((await fetch(first.url+'/v1/repl')).status,200);assert.equal((await fetch(second.url+'/v1/repl')).status,200)
   for(const item of [a,b]){const done=once(item.child,'exit');item.child.stdin.end('/exit\n');await done;assert.equal(item.child.exitCode,0)}
   const reopened=launch('alpha');await wait(reopened,'group>');const done=once(reopened.child,'exit');reopened.child.stdin.end('/exit\n');await done
 }finally{
   for(const item of children){if(item.child.exitCode===null&&item.child.signalCode===null){const done=once(item.child,'exit');item.child.kill('SIGTERM');await done}}
   await model.close();await rm(dir,{recursive:true,force:true})
 }
})
