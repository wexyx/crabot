import test from 'node:test'
import assert from 'node:assert/strict'
import {spawn} from 'node:child_process'
import {once} from 'node:events'
import {mkdtemp,readFile,writeFile,rm,access} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join,resolve} from 'node:path'
import {pause,start,stop,request} from './admin-fixture.mjs'

test('startup inherits release defaults without copying; Web, CLI and text edits share overrides',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'crabot-prompts-'))
 const otherDir=await mkdtemp(join(tmpdir(),'crabot-prompts-other-'))
 let child,other,output=''
 const wait=async text=>{for(let i=0;i<300;i++){if(output.includes(text))return;if(child.exitCode!==null)break;await pause(20)}throw Error('CLI timeout: '+output)}
 const launch=async()=>{
  output=''
  child=spawn(resolve('target/debug/agent-node'),['--cli','--data-dir',dir,'--server-port','0'],{cwd:dir,env:{PATH:process.env.PATH,ADMIN_AGENT_PROVIDER:'mock',NODE_LINKS_JSON:'[]'},stdio:['pipe','pipe','pipe']})
  child.stdout.on('data',v=>output+=v);child.stderr.on('data',v=>output+=v)
  await wait('group>')
  return {url:output.match(/http:\/\/127\.0\.0\.1:\d+/)[0]}
 }
 const close=async()=>{const done=once(child,'exit');child.stdin.end('/exit\n');await done;assert.equal(child.exitCode,0)}
 try{
  let server=await launch()
  const file=join(dir,'conf','discussion.md')
  await assert.rejects(access(join(dir,'conf')), {code:'ENOENT'})
  const data=await request(server,'/v1/config/prompts')
  const original=data.prompts.find(p=>p.id==='discussion').default
  assert.ok(data.prompts.every(p=>!p.overridden&&p.content===p.default))
  await assert.rejects(access(join(dir,'conf')), {code:'ENOENT'})
  assert.equal(data.directory,join(dir,'conf'));assert.equal(data.prompts.length,9)
  const put=(body,origin)=>fetch(server.url+'/v1/config/prompts',{method:'PUT',headers:{'content-type':'application/json',...(origin?{origin}:{})},body:JSON.stringify(body)})
  const saved=await put({id:'discussion',expected:original,content:'Web custom discussion'})
  assert.equal(saved.status,200);assert.equal(await readFile(file,'utf8'),'Web custom discussion')
  assert.equal((await request(server,'/v1/config/prompts')).prompts.find(p=>p.id==='discussion').overridden,true)
  output='';child.stdin.write('/prompts show discussion\n');await wait('Web custom discussion')
  output='';child.stdin.write('/prompts set discussion CLI custom discussion\n');await wait('已保存 discussion')
  assert.equal((await request(server,'/v1/config/prompts')).prompts.find(p=>p.id==='discussion').content,'CLI custom discussion')
  await writeFile(file,'Editor custom discussion\n','utf8')
  assert.equal((await request(server,'/v1/config/prompts')).prompts.find(p=>p.id==='discussion').content,'Editor custom discussion\n')
  assert.notEqual((await put({id:'discussion',expected:'CLI custom discussion',content:'stale'})).status,200)
  assert.notEqual((await put({id:'../secret',expected:'',content:'invalid'})).status,200)
  assert.equal((await put({id:'discussion',expected:'Editor custom discussion\n',content:'attack'},'https://evil.example')).status,403)
  const imported=join(dir,'input.md');await writeFile(imported,'Line one\nLine two\n')
  output='';child.stdin.write(`/prompts import discussion ${imported}\n`);await wait('已保存 discussion')
  assert.equal(await readFile(file,'utf8'),'Line one\nLine two\n')
  other=await start(otherDir,{ADMIN_AGENT_PROVIDER:'mock'})
  assert.equal((await request(other,'/v1/config/prompts')).prompts.find(p=>p.id==='discussion').content,original)
  await close();server=await launch()
  assert.equal(await readFile(file,'utf8'),'Line one\nLine two\n','restart must preserve edits')
  output='';child.stdin.write('/prompts reset discussion\n');await wait('已恢复 discussion')
  await assert.rejects(access(file),{code:'ENOENT'})
  const reset=(await request(server,'/v1/config/prompts')).prompts.find(p=>p.id==='discussion')
  assert.equal(reset.content,original);assert.equal(reset.overridden,false)
  assert.equal((await put({id:'discussion',expected:original,content:original})).status,200)
  assert.equal((await request(server,'/v1/config/prompts')).prompts.find(p=>p.id==='discussion').overridden,true,'an explicit copy is still an override')
  assert.equal((await put({id:'discussion',expected:original,reset:true})).status,200)
  await assert.rejects(access(file),{code:'ENOENT'})
  await close()
  await launch()
  await assert.rejects(access(file),{code:'ENOENT'})
  await close()
 }finally{
  if(child?.exitCode===null&&child.signalCode===null){const done=once(child,'exit');child.kill('SIGTERM');await done}
  await stop(other)
  await rm(dir,{recursive:true,force:true});await rm(otherDir,{recursive:true,force:true})
 }
})
