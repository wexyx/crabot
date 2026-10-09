import {readState} from './state-fixture.mjs'
import test from 'node:test'
import assert from 'node:assert/strict'
import {spawn} from 'node:child_process'
import {once} from 'node:events'
import {mkdtemp,readFile,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join,resolve} from 'node:path'
import {modelFixture,pause} from './admin-fixture.mjs'
test('CLI starts clean, history replays on request, and Web reuses the saved port',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-cli-history-')),model=await modelFixture();let item
 const launch=(args=[])=>{
  const child=spawn(resolve('target/debug/agent-node'),['--cli',...args],{cwd:dir,env:{PATH:process.env.PATH,CRABOT_DATA_DIR:dir,...model.env},stdio:['pipe','pipe','pipe']})
  const result={child,output:''};child.stdout.on('data',b=>result.output+=b);child.stderr.on('data',b=>result.output+=b);return result
 }
 const wait=async text=>{for(let i=0;i<200;i++){if(item.output.includes(text))return;if(item.child.exitCode!==null)throw Error(item.output);await pause(20)}throw Error('missing '+text+': '+item.output)}
 const exit=async()=>{const done=once(item.child,'exit');item.child.stdin.end('/exit\n');await done;assert.equal(item.child.exitCode,0)}
 const web=async()=>{item.output='';item.child.stdin.write('启动 Web TEST_PLAN:[{"name":"server_start","input":{}}]\n');await wait('Fixture complete\nadmin> ');const state=await readState(dir);return state.collections.instance_settings.web.address}
 try{
  item=launch(['--web-port','0']);await wait('group> ');item.child.stdin.write('/manage\n');await wait('admin> ')
  item.output='';item.child.stdin.write('persisted-history-marker TEST_PLAN:[]\n');await wait('Fixture complete\nadmin> ')
  const address=await web();assert.ok(!address.endsWith(':0'));await exit()
  item=launch();await wait('group> ');item.child.stdin.write('/manage\n');await wait('admin> ');await pause(100);assert.ok(!item.output.includes('persisted-history-marker'));assert.ok(!item.output.includes('Fixture complete'))
  item.output='';item.child.stdin.write('/history\n');await wait('persisted-history-marker');await wait('Fixture complete\nadmin> ')
  assert.equal(await web(),address);await exit()
 }finally{
  if(item?.child.exitCode===null&&item.child.signalCode===null){const done=once(item.child,'exit');item.child.kill('SIGTERM');await done}
  await model.close();await rm(dir,{recursive:true,force:true})
 }
})
