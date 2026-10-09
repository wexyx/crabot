import test from 'node:test'
import assert from 'node:assert/strict'
import {spawn} from 'node:child_process'
import {once} from 'node:events'
import {mkdtemp,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join,resolve} from 'node:path'
import {pause,request,policy} from './admin-fixture.mjs'
import {readState} from './state-fixture.mjs'

test('CLI and Web share Agent/project/capability state; CLI works with Server stopped',{timeout:30000},async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-workbench-cli-'))
 const child=spawn(resolve('target/debug/agent-node'),['--cli','--data-dir',dir,'--server-port','0'],{cwd:dir,env:{PATH:process.env.PATH,ADMIN_AGENT_PROVIDER:'mock',NODE_LINKS_JSON:'[]'},stdio:['pipe','pipe','pipe']})
 let output=''
 child.stdout.on('data',v=>output+=v);child.stderr.on('data',v=>output+=v)
 async function wait(predicate){for(let i=0;i<250;i++){if(predicate())return;if(child.exitCode!==null)throw Error(output);await pause(20)}throw Error(output)}
 async function command(line,marker){const offset=output.length;child.stdin.write(line+'\n');await wait(()=>output.slice(offset).includes(marker));return output.slice(offset)}
 try{
  await wait(()=>output.includes('group>'))
  const state=await readState(dir)
  const server={url:'http://'+state.collections.instance_settings.web.address}
  const p=(await request(server,'/v1/repl')).projects[0].id,base='/v1/repl/'+p
  await command('/agent add parity mock CLI 创建','CLI 创建')
  let row=(await request(server,base+'/agents')).agents.find(a=>a.id==='parity')
  assert.equal(row.role,'CLI 创建')
  let response=await fetch(server.url+base+'/agents',{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({client_id:'parity',provider:'mock',role:'Web 编辑',expected_version:row.version})})
  assert.equal(response.status,200,await response.text())
  assert.match(await command('/agent show parity','Web 编辑'),/Web 编辑/)
  await command('/agent start parity','started')
  await command('/project create '+JSON.stringify({name:'同一项目',policy:{...policy('parity'),mode:'chat'}}),'同一项目')
  const group=(await request(server,'/v1/repl')).collaboration_projects.find(g=>g.body.name==='同一项目')
  assert.ok(group)
  assert.match(await command('/project',group.key),/同一项目/)
  for(const entry of ['/chat','/sessions']){
   const listing=await command(entry,group.key)
   assert.match(listing,/同一项目/);assert.doesNotMatch(listing,/error:/)
  }
  await command('/chat\t'+group.key.slice(0,8),group.key.slice(0,8))
  assert.match(await command('/members','parity'),/tester/)
  const directory=await command('/agents','parity');assert.match(directory,/default/)
  await command('/manage','admin>')
  await command('/back','已返回')
  assert.match(await command('/chat missing-project','找不到项目'),/\/chat/)
  assert.match(await command('/chat 同一项目',group.key.slice(0,8)),/group/)
  assert.match(await command('/history 同一项目','已恢复最近的聊天记录'),/已恢复/)
  await command('/new','已开启新上下文')
  await command('/chat','＋ 新建聊天')
  await command('/cancel','已取消选择聊天')
  const beforeNew=(await request(server,'/v1/repl')).collaboration_projects
  await command('/chat','＋ 新建聊天')
  await command('1','group>')
  const afterNew=(await request(server,'/v1/repl')).collaboration_projects
  const fresh=afterNew.find(row=>!beforeNew.some(old=>old.key===row.key))
  assert.ok(fresh,'picker must create a new chat, not reuse the startup chat')
  assert.equal(fresh.body.policy.mode,'chat')
  assert.deepEqual(fresh.body.policy.members[0].path,['default'])
  await command('/back','已返回')
  await command('/namespace','已废弃')
  await command('/new unexpected','用法：/new')
  await command('/unknown','未知命令')
  assert.match(await command('/help commands','/exit'),/\/chat/)
  await command('/manage','admin')
  await command('/agent test parity','已进入 Agent 测试聊天')
  await command('测试共享会话','Mock')
  assert.ok(!(await request(server,'/v1/repl')).collaboration_projects.some(g=>g.body.kind==='agent_test'))
  await command('/manage','admin')
  assert.match(await command('/capabilities list {"scope":"business","kind":"tool"}','shell'),/shell/)
  assert.match(await command('/help network','/connect URL'),/人工|确认/)
  await command('/server stop','stopped')
  await command('/agent add offline mock 不依赖HTTP','不依赖HTTP')
  await command('/agent show offline','不依赖HTTP')
  await command('/agent delete offline 1 --confirm','deleted')
  const exit=once(child,'exit');child.stdin.end('/exit\n');await exit;assert.equal(child.exitCode,0)
 }finally{
  if(child.exitCode===null&&child.signalCode===null){const exit=once(child,'exit');child.kill('SIGTERM');await exit}
  await rm(dir,{recursive:true,force:true})
 }
})
