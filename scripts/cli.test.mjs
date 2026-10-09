import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,mkdir,copyFile,writeFile,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {spawnSync} from 'node:child_process'
async function fixture(fn){
  const dir=await mkdtemp(join(tmpdir(),'crabot-cli-test-'))
  try{
    for(const file of ['agent-node','crabot'])await copyFile(file,join(dir,file))
    await mkdir(join(dir,'bin'));await mkdir(join(dir,'apps/web/dist'),{recursive:true})
    await writeFile(join(dir,'apps/web/dist/index.html'),'<html></html>')
    await writeFile(join(dir,'bin/cargo'),'#!/bin/sh\nprintf "NATIVE %s %s %s\\n" "$*" "$AGENT_MODE" "$BIND_ADDR"\n',{mode:0o755})
    const run=(args=[],env={},entry='agent-node')=>spawnSync('/bin/bash',[join(dir,entry),...args],{cwd:dir,env:{PATH:join(dir,'bin')+':/usr/bin:/bin',...env},encoding:'utf8'})
    await fn({dir,run})
  }finally{await rm(dir,{recursive:true,force:true})}
}
test('native server and interactive launchers are distinct',async()=>fixture(async({run})=>{
  for(const args of [[],['start'],['run']]){
    const r=run(args);assert.equal(r.status,0,r.stderr);assert.match(r.stdout,/NATIVE run -p agent-node --bin agent-node/);assert.doesNotMatch(r.stdout,/--cli/)
  }
  const cli=run([],{},'crabot');assert.equal(cli.status,0,cli.stderr);assert.match(cli.stdout,/--cli/)
}))
test('retired deployment and management commands are rejected',async()=>fixture(async({run})=>{
  for(const args of [['docker-start'],['start','--docker'],['peer-setup'],['setup'],['runtime-setup']])assert.equal(run(args).status,2)
  const proxy=run(['proxy']);assert.equal(proxy.status,0,proxy.stderr);assert.match(proxy.stdout,/NATIVE.*proxy/)
}))
test('launcher leaves dotenv parsing to Rust and preserves explicit environment',async()=>fixture(async({dir,run})=>{
  await writeFile(join(dir,'.env'),'DATABASE_URL=retired\nBIND_ADDR=127.0.0.1:1111\n')
  await writeFile(join(dir,'.crabot.env'),'BIND_ADDR=127.0.0.1:2222\n')
  assert.doesNotMatch(run().stdout,/127\.0\.0\.1:2222/)
  assert.match(run([],{BIND_ADDR:'127.0.0.1:3333'}).stdout,/127\.0\.0\.1:3333/)
  assert.equal(run(['--outside-access','bad']).status,2)
}))
