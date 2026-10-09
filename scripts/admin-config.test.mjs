import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,readFile,rm,stat} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join,resolve} from 'node:path'
import {spawnSync} from 'node:child_process'
import {start,stop,request} from './admin-fixture.mjs'

test('human configuration requires authentication, masks secrets and survives restart',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'crabot-admin-config-'))
  let server
  try{
    server=await start(dir,{ADMIN_AGENT_PROVIDER:'mock'})
    const path='/v1/admin-agent/configuration'
    assert.equal((await fetch(server.url+path,{headers:{origin:'https://evil.example'}})).status,403)
    const put=body=>fetch(server.url+path,{method:'PUT',headers:{'x-admin-token':server.token,'content-type':'application/json'},body:JSON.stringify(body)})
    assert.equal((await put({UNKNOWN:'value'})).status,400)
    let response=await put({ADMIN_AGENT_PROVIDER:'crabot',MODEL_PROVIDER:'ollama',MODEL_NAME:'fixture',MODEL_API_KEY:'secret-fixture',MODEL_API:'chat',MODEL_BASE_URL:'http://localhost:11434/v1'})
    assert.ok(response.ok,await response.text())
    const project=(await request(server,"/v1/repl")).projects[0].id
    const defaultAgent=(await request(server,"/v1/repl/"+project+"/agents")).agents.find(a=>a.is_default)
    assert.equal(defaultAgent.provider,"crabot")
    assert.equal(defaultAgent.online,true)
    const view=await request(server,path)
    assert.equal(view.configuration.has_api_key,true)
    assert.ok(!JSON.stringify(view).includes('secret-fixture'))
    const file=join(dir,'default-agent.json')
    const saved=JSON.parse(await readFile(file,'utf8'))
    assert.equal(saved.MODEL_API_KEY,'secret-fixture')
    assert.equal((await stat(file)).mode&0o777,0o600)
    response=await put({ADMIN_AGENT_PROVIDER:'crabot',MODEL_PROVIDER:'invalid'})
    assert.equal(response.status,409)
    assert.deepEqual(JSON.parse(await readFile(file,'utf8')),saved)
    await stop(server)
    server=await start(dir,{ADMIN_AGENT_PROVIDER:undefined,MODEL_PROVIDER:undefined,MODEL_NAME:undefined,MODEL_API_KEY:undefined,MODEL_API:undefined,MODEL_BASE_URL:undefined})
    const loaded=await request(server,path)
    assert.equal(loaded.configuration.values.MODEL_PROVIDER,'ollama')
    assert.equal(loaded.configuration.has_api_key,true)
    await stop(server)
    server=await start(dir,{ADMIN_AGENT_PROVIDER:undefined,MODEL_PROVIDER:'invalid-template',MODEL_NAME:'',MODEL_API_KEY:'',MODEL_API:'',MODEL_BASE_URL:'',CRABOT_CONFIG_DEFAULT_KEYS:' MODEL_PROVIDER MODEL_NAME MODEL_API_KEY MODEL_API MODEL_BASE_URL '})
    const retained=await request(server,path)
    assert.equal(retained.configuration.values.MODEL_PROVIDER,'ollama')
    assert.equal(retained.configuration.has_api_key,true)
  }finally{await stop(server);await rm(dir,{recursive:true,force:true})}
})

test('noninteractive invalid startup fails clearly without waiting for stdin',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'crabot-admin-missing-'))
  try{
    const result=spawnSync(resolve('target/debug/agent-node'),['--cli'],{cwd:dir,env:{PATH:process.env.PATH,CRABOT_DATA_DIR:dir,MODEL_PROVIDER:'invalid'},encoding:'utf8',timeout:5000})
    assert.equal(result.status,2)
    assert.match(result.stderr,/请在终端运行 crabot 完成配置/)
    assert.doesNotMatch(result.stderr,/AdminAgent/)
    assert.ok(!result.stdout.includes('Local data:'))
  }finally{await rm(dir,{recursive:true,force:true})}
})

test('CLI can reconfigure after startup without sending settings to the model',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'crabot-admin-repl-'))
  try{
    const result=spawnSync(resolve('target/debug/agent-node'),['--cli','--web-port','0'],{cwd:dir,env:{PATH:process.env.PATH,CRABOT_DATA_DIR:dir,ADMIN_AGENT_PROVIDER:'mock'},input:'/admin-config\ncodex\n/usr/bin/false\n{}\n/exit\n',encoding:'utf8',timeout:10000})
    assert.equal(result.status,0,result.stderr)
    assert.match(result.stdout,/配置已保存并生效/)
    const saved=JSON.parse(await readFile(join(dir,'default-agent.json'),'utf8'))
    assert.equal(saved.ADMIN_AGENT_PROVIDER,'codex')
    assert.equal(saved.CODEX_BIN,'/usr/bin/false')
  }finally{await rm(dir,{recursive:true,force:true})}
})
