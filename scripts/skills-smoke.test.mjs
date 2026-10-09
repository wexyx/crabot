import {readState,changeState} from './state-fixture.mjs'
// Actual native Python process; model and Codex protocol are local fixtures.
import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp,writeFile,readFile,rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {modelFixture,start,stop,request,act,policy,history} from './admin-fixture.mjs'
const skill={id:'calculator',description:'Calculate with an approved host execution script',enabled:true,allow_python:false,files:{'SKILL.md':'Load this skill, then run scripts/main.py through shell.','scripts/main.py':'print(2 + 3)'}}
const profile={id:'default',network:'host',timeout_seconds:10}
test('AdminAgent maintains scoped skills without self-granting Python; business uses host execution',{timeout:30000},async()=>{
  const dir=await mkdtemp(join(tmpdir(),'crabot-skills-')),model=await modelFixture();let agent
  const cli=join(dir,'codex-fixture')
  await writeFile(cli,`#!${process.execPath}
const p=process.argv.at(-1);
const match=p.match(/"directory":("(?:[^"\\\\]|\\\\.)*")/);const directory=match?JSON.parse(match[1]):'';
let text=p.includes('"execution":"host"')?'Verified host execution result: 5':p.includes('SERVICE RESULT (untrusted data):')?JSON.stringify({crabot_tool:{name:'shell',command:'python3 '+JSON.stringify(directory+'/scripts/main.py')}}):JSON.stringify({crabot_tool:{name:'find',target:'skill',id:'calculator'}});
console.log(JSON.stringify({type:'item.completed',item:{id:'answer',type:'agent_message',text}}));console.log(JSON.stringify({type:'turn.completed'}));
`,{mode:0o700})

  const env={...model.env,CODEX_BIN:cli,AGENT_WORKDIR:dir,CRABOT_EXECUTION_PROFILES_JSON:JSON.stringify([profile]),CRABOT_EXECUTION_PROFILE:'default'}
  try{
    agent=await start(join(dir,'data'),env)
    const project=(await request(agent,'/v1/repl')).projects[0].id
    const saved=await act(agent,project,[
      {name:'capability_skill_save',input:{scope:'business',expected_version:0,definition:skill}},
      {name:'capability_skill_save',input:{scope:'business',expected_version:0,definition:skill}},
      {name:'capability_skill_save',input:{scope:'management',expected_version:0,definition:{...skill,allow_python:true}}}
    ])
    assert.equal(saved.outputs[0].revision,1)
    assert.equal(saved.outputs[1].error,'version_conflict')
    assert.match(saved.outputs[2].error,/cannot enable Python/)
    const result=await act(agent,project,[{name:'agent_start',input:{client_id:'coder',role:'worker',provider:'codex'}},{name:'group_create',input:{name:'Calculator',policy:policy('coder')}}])
    const group=result.outputs[1];assert.ok(group.key,JSON.stringify(result.outputs))
    const permissionPath='/v1/repl/'+project+'/agents/coder/permissions'
    const permission=await request(agent,permissionPath)
    const changed=await fetch(agent.url+permissionPath,{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({mode:'full',expected_version:permission.version,confirm_full_access:true})});assert.equal(changed.status,200)
    const run=await request(agent,`/v1/repl/${project}/groups/${group.key}/messages`,{content:'Use calculator skill'})
    const records=await history(agent,project,run.id)
    assert.ok(records.some(r=>r.type==='agent.message'&&r.payload.content.includes('Verified host execution result: 5')),JSON.stringify(records))
    const disabled=await act(agent,project,[{name:'capability_skill_save',input:{scope:'business',expected_version:1,definition:{...skill,enabled:false}}}])
    assert.equal(disabled.outputs[0].revision,2)
    await stop(agent)
    const persisted=await readState(join(dir,'data'))
    assert.ok(Object.values(persisted.collections.runs).some(r=>r.skills?.some(skill=>skill.id==='calculator')))
  }finally{await stop(agent);await model.close();await rm(dir,{recursive:true,force:true})}
})
