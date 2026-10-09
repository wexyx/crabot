import test from 'node:test'
import assert from 'node:assert/strict'
import {spawn} from 'node:child_process'
import {once} from 'node:events'
import {mkdtemp,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join,resolve} from 'node:path'
import {pause,request} from './admin-fixture.mjs'

test('CLI opens one persistent simple chat and enters management only by command',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-default-chat-'));let child,output=''
 const wait=async text=>{for(let i=0;i<250;i++){if(output.includes(text))return;await pause(20)}throw Error(output)}
 const close=async()=>{const ended=once(child,'exit');child.stdin.end('/exit\n');await ended;assert.equal(child.exitCode,0)}
 const launch=async()=>{
  output='';child=spawn(resolve('target/debug/agent-node'),['--cli','--data-dir',dir,'--server-port','0'],{cwd:dir,env:{PATH:process.env.PATH,ADMIN_AGENT_PROVIDER:'mock',NODE_LINKS_JSON:'[]'},stdio:['pipe','pipe','pipe']})
  child.stdout.on('data',v=>output+=v);child.stderr.on('data',v=>output+=v)
  await wait('group>');return {url:output.match(/(http:\/\/127\.0\.0\.1:\d+)/)[1]}
 }
 try{
  let server=await launch()
  const first=(await request(server,'/v1/repl')).collaboration_projects
  assert.equal(first.length,1);assert.equal(first[0].body.policy.mode,'chat')
  assert.deepEqual(first[0].body.policy.members.map(m=>m.path),[['default']])
  child.stdin.write('persistent-simple-chat-marker\n');await wait('Mock')
  output='';child.stdin.write('/manage\n');await wait('admin>')
  output='';child.stdin.write('/admin\n');await wait('admin>')
  output='';child.stdin.write('/back\n');await wait('group>');await wait('persistent-simple-chat-marker')
  output='';child.stdin.write('/back\n');await wait('没有上一个聊天')
  await close()
  server=await launch()
  const next=(await request(server,'/v1/repl')).collaboration_projects
  assert.equal(next.length,1);assert.equal(next[0].key,first[0].key)
  assert.ok(!output.includes('persistent-simple-chat-marker'))
  child.stdin.write('/history\n');await wait('persistent-simple-chat-marker')
  output='';child.stdin.write('/new\n');await wait('已开启新上下文');await wait('group>')
  const logs=await request(server,`/v1/repl/${next[0].namespace_id}/chats/${next[0].key}/history?limit=100`)
  assert.match(JSON.stringify(logs),/context.reset/)
  assert.match(JSON.stringify(logs),/persistent-simple-chat-marker/)
  await close()
 }finally{if(child?.exitCode===null&&child.signalCode===null){const ended=once(child,'exit');child.kill('SIGTERM');await ended}await rm(dir,{recursive:true,force:true})}
})
