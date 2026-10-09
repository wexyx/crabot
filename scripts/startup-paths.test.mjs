import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,mkdir,rm,realpath,writeFile} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join,resolve} from 'node:path'
import {spawn} from 'node:child_process'
import {once} from 'node:events'
import {pause,stop} from './admin-fixture.mjs'

const binary=resolve('target/debug/agent-node')
test('startup paths use home, preserve overrides and lock the shared instance',{timeout:30000},async()=>{
  const temp=await realpath(await mkdtemp(join(tmpdir(),'crabot-paths-')))
  const home=join(temp,'home'),cwd=join(temp,'launch'),work=join(temp,'work')
  await Promise.all([home,cwd,work].map(p=>mkdir(p)))
  const base={...process.env,HOME:home,USERPROFILE:home,ADMIN_AGENT_PROVIDER:'mock',BIND_ADDR:'127.0.0.1:0',NODE_LINKS_JSON:'[]'}
  delete base.CRABOT_DATA_DIR;delete base.AGENT_WORKDIR
  async function launch(args=[],overrides={},directory=cwd){
    const child=spawn(binary,args,{cwd:directory,env:{...base,...overrides},stdio:['ignore','pipe','pipe']})
    let output='';child.stdout.on('data',b=>output+=b);child.stderr.on('data',b=>output+=b)
    for(let i=0;i<300;i++){
      const url=output.match(/Server: (http:\/\/\S+)/)?.[1]
      if(url)return {child,url,output}
      if(child.exitCode!==null)throw new Error(output)
      await pause(20)
    }
    child.kill('SIGKILL');throw Error(output)
  }
  let server
  try{
    for(const [args,env,data,root] of [
      [[],{},join(home,'.crabot'),home],
      [['--name','dev','--workdir',work],{},join(home,'.crabot_dev'),work],
      [[],{CRABOT_DATA_DIR:join(temp,'env-data'),AGENT_WORKDIR:work},join(temp,'env-data'),work],
      [['--data-dir',join(temp,'cli-data'),'--workdir',home],{CRABOT_DATA_DIR:join(temp,'ignored'),AGENT_WORKDIR:work},join(temp,'cli-data'),home],
      [['--name','named'],{CRABOT_DATA_DIR:join(temp,'ignored')},join(home,'.crabot_named'),home],
    ]){
      server=await launch(args,env)
      assert.ok(server.output.includes('Local data: '+data),server.output)
      const config=await fetch(server.url+'/v1/workspace').then(r=>r.json())
      assert.equal(config.workdir,root)
      await stop(server);server=null
    }
    // Instance defaults, launch-directory override, explicit environment and CLI priority.
    await writeFile(join(home,'.crabot','.agent.env'), 'AGENT_WORKDIR='+work+'\n')
    server=await launch()
    assert.equal((await fetch(server.url+'/v1/workspace').then(r=>r.json())).workdir,work)
    await stop(server)
    await writeFile(join(cwd,'.agent.env'),'AGENT_WORKDIR='+home+'\n')
    server=await launch()
    assert.equal((await fetch(server.url+'/v1/workspace').then(r=>r.json())).workdir,home)
    await stop(server)
    server=await launch([], {AGENT_WORKDIR:work})
    assert.equal((await fetch(server.url+'/v1/workspace').then(r=>r.json())).workdir,work)
    await stop(server)
    server=await launch(['--workdir',home], {AGENT_WORKDIR:work})
    assert.equal((await fetch(server.url+'/v1/workspace').then(r=>r.json())).workdir,home)
    await stop(server)
    await writeFile(join(home,'.crabot_dev','.agent.env'),'AGENT_WORKDIR='+work+'\n')
    server=await launch(['--name','dev'],{},work)
    assert.equal((await fetch(server.url+'/v1/workspace').then(r=>r.json())).workdir,work)
    await stop(server)
    server=await launch()
    await assert.rejects(launch([],{},work),/already|lock|running|in use/i)
  }finally{await stop(server);await rm(temp,{recursive:true,force:true})}
})
