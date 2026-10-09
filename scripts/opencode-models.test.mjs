import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,mkdir,writeFile,chmod,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {start,stop,request} from './admin-fixture.mjs'

const path='/v1/opencode/models'

// A fake CLI so the test never depends on which OpenCode the machine happens to have.
async function fakeCli(dir,body){
  const bin=join(dir,'bin')
  await mkdir(bin,{recursive:true})
  const file=join(bin,'opencode')
  await writeFile(file,body)
  await chmod(file,0o755)
  return join(bin,'opencode')
}
const priced=`#!/bin/sh
case "$*" in
  *"api get /api/model"*) cat <<'JSON'
{"location":{},"data":[
 {"id":"paid","providerID":"opencode","name":"Paid","cost":[{"input":2,"output":10,"cache":{"read":0.1,"write":2.5}}],"limit":{"context":200000},"capabilities":{"tools":true},"status":"active"},
 {"id":"free-model","providerID":"opencode","name":"Free","cost":[{"input":0,"output":0,"cache":{"read":0,"write":0}}],"limit":{"context":100000},"capabilities":{"tools":true},"status":"active"}
]}
JSON
  ;;
  *) exit 2 ;;
esac
`
const identifiersOnly=`#!/bin/sh
case "$*" in
  *"api get /api/model"*) echo "unavailable" >&2; exit 1 ;;
  models) printf 'opencode/alpha\nopencode/beta\n' ;;
  *) exit 2 ;;
esac
`
const noModels=`#!/bin/sh
case "$*" in
  *"api get /api/model"*) echo '{"location":{},"data":[]}' ;;
  *) exit 2 ;;
esac
`

test('the picker is offered the priced catalog, free models first',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'crabot-oc-models-'))
  let server
  try{
    server=await start(dir,{ADMIN_AGENT_PROVIDER:'opencode',OPENCODE_BIN:await fakeCli(dir,priced)})
    const catalog=await request(server,path)
    assert.equal(catalog.source,'catalog')
    assert.equal(catalog.priced,true)
    assert.equal(catalog.signed_in,true)
    assert.deepEqual(catalog.models.map(m=>m.id),['opencode/free-model','opencode/paid'])
    assert.equal(catalog.models[0].free,true)
    assert.equal(catalog.models[1].input,2)
  }finally{await stop(server);await rm(dir,{recursive:true,force:true})}
})

test('a CLI that cannot price models still lists them without calling them free',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'crabot-oc-fallback-'))
  let server
  try{
    server=await start(dir,{ADMIN_AGENT_PROVIDER:'opencode',OPENCODE_BIN:await fakeCli(dir,identifiersOnly)})
    const catalog=await request(server,path)
    assert.equal(catalog.source,'identifiers')
    assert.equal(catalog.priced,false)
    assert.deepEqual(catalog.models.map(m=>m.id),['opencode/alpha','opencode/beta'])
    for(const model of catalog.models)assert.equal(model.free,false,`${model.id} claimed free`)
  }finally{await stop(server);await rm(dir,{recursive:true,force:true})}
})

test('a signed-out OpenCode is reported as such instead of an empty picker',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'crabot-oc-signedout-'))
  let server
  try{
    server=await start(dir,{ADMIN_AGENT_PROVIDER:'opencode',OPENCODE_BIN:await fakeCli(dir,noModels)})
    const catalog=await request(server,path)
    assert.deepEqual(catalog.models,[])
    assert.equal(catalog.signed_in,false)
  }finally{await stop(server);await rm(dir,{recursive:true,force:true})}
})

// Discovery asks the local CLI, so it must work while the operator is still
// choosing a runner and while a per-Agent override names its own launcher.
test('the picker works whatever runner is currently saved',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'crabot-oc-othrunner-'))
  let server
  try{
    const opencode=await fakeCli(dir,priced)
    // crabot additionally demands a model name, which discovery must not depend on.
    for(const provider of ['mock','codex','claude','opencode']){
      server=await start(dir,{ADMIN_AGENT_PROVIDER:provider,OPENCODE_BIN:opencode})
      const catalog=await request(server,path)
      assert.equal(catalog.models.length,2,`runner ${provider} refused discovery`)
      await stop(server)
    }
    // crabot also demands its own credentials, none of which discovery may need.
    server=await start(dir,{ADMIN_AGENT_PROVIDER:'crabot',MODEL_PROVIDER:'ollama',MODEL_NAME:'fixture',OPENCODE_BIN:opencode})
    assert.equal((await request(server,path)).models.length,2,'runner crabot refused discovery')
  }finally{await stop(server);await rm(dir,{recursive:true,force:true})}
})

test('a per-Agent launcher overrides the saved one for discovery',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'crabot-oc-agentbin-'))
  let server
  try{
    server=await start(dir,{ADMIN_AGENT_PROVIDER:'opencode',OPENCODE_BIN:await fakeCli(dir,priced)})
    const override=join(dir,'bin','other-opencode')
    await writeFile(override,priced.replace('free-model','override-free'))
    await chmod(override,0o755)
    const catalog=await request(server,path+'?bin='+encodeURIComponent(override))
    assert.ok(catalog.models.some(m=>m.id==='opencode/override-free'),'the override launcher was ignored')
    // Without the override the saved launcher is still what answers.
    assert.ok(!(await request(server,path)).models.some(m=>m.id==='opencode/override-free'))
  }finally{await stop(server);await rm(dir,{recursive:true,force:true})}
})

test('a missing launcher is reported as a bad gateway, not a runner error',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'crabot-oc-nolauncher-'))
  let server
  try{
    server=await start(dir,{ADMIN_AGENT_PROVIDER:'mock'})
    const response=await fetch(server.url+path+'?bin='+encodeURIComponent('/nonexistent/opencode'),{headers:{'x-admin-token':server.token}})
    assert.equal(response.status,502)
    assert.match((await response.json()).error,/无法执行/)
  }finally{await stop(server);await rm(dir,{recursive:true,force:true})}
})

test('a saved model survives the runner switch that a picker implies',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'crabot-oc-saved-'))
  let server
  try{
    server=await start(dir,{ADMIN_AGENT_PROVIDER:'opencode',OPENCODE_BIN:await fakeCli(dir,priced)})
    const config='/v1/admin-agent/configuration'
    const put=body=>fetch(server.url+config,{method:'PUT',headers:{'x-admin-token':server.token,'content-type':'application/json'},body:JSON.stringify(body)})
    const saved=await put({OPENCODE_MODEL:'opencode/free-model'})
    assert.ok(saved.ok,await saved.text())
    const view=await request(server,config)
    assert.equal(view.configuration.values.OPENCODE_MODEL,'opencode/free-model')
    // The saved model must reach the runner, not just the stored settings.
    assert.equal((await request(server,path)).models.length,2)
    await stop(server)
    server=await start(dir,{ADMIN_AGENT_PROVIDER:undefined,OPENCODE_BIN:undefined})
    assert.equal((await request(server,config)).configuration.values.OPENCODE_MODEL,'opencode/free-model')
  }finally{await stop(server);await rm(dir,{recursive:true,force:true})}
})
