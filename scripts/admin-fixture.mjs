import {createServer} from 'node:http'
import {spawn} from 'node:child_process'
import {once} from 'node:events'
import {readFile,mkdir} from 'node:fs/promises'
import {join,resolve} from 'node:path'
import assert from 'node:assert/strict'
export const pause=ms=>new Promise(r=>setTimeout(r,ms))
export async function modelFixture(){
  const server=createServer(async(req,res)=>{
    let text='';for await(const chunk of req)text+=chunk
    const body=JSON.parse(text)
    const prompt=body.messages.find(m=>m.role==='user')?.content || ''
    const plan=JSON.parse(prompt.slice(prompt.lastIndexOf('TEST_PLAN:')+10))
    const index=body.messages.filter(m=>m.role==='tool').length
    const item=plan[index]
    const chunk=item?{choices:[{delta:{tool_calls:[{index:0,id:'call-'+index,type:'function',function:{name:item.name,arguments:JSON.stringify(item.input)}}]},finish_reason:'tool_calls'}]}:{choices:[{delta:{content:'Fixture complete'},finish_reason:'stop'}]}
    res.writeHead(200,{'content-type':'text/event-stream'});res.end('data: '+JSON.stringify(chunk)+'\n\ndata: [DONE]\n\n')
  })
  server.listen(0,'127.0.0.1');await once(server,'listening')
  return {env:{MODEL_PROVIDER:'compatible',MODEL_API:'chat',MODEL_BASE_URL:`http://127.0.0.1:${server.address().port}/v1`,MODEL_NAME:'fixture',MODEL_API_KEY:'fixture',ADMIN_AGENT_PROVIDER:'crabot'},close:()=>new Promise(r=>server.close(r))}
}
export async function start(dir,env={}){
  await mkdir(dir,{recursive:true})
  const child=spawn(resolve('target/debug/agent-node'),[],{cwd:dir,env:{...process.env,CRABOT_DATA_DIR:dir,AGENT_WORKDIR:resolve('.'),BIND_ADDR:'127.0.0.1:0',ADMIN_TOKEN:'fixture-admin-token',AGENT_MODE:'agent',NODE_LINKS_JSON:'[]',OPENAI_API_KEY:'fixture',ANTHROPIC_API_KEY:'fixture',...env},stdio:['ignore','pipe','pipe']})
  let output='';child.stdout.on('data',b=>output+=b);child.stderr.on('data',b=>output+=b)
  for(let i=0;i<240;i++){
    const match=output.match(/(?:Server|Web REPL): (http:\/\/\S+)/)
    if(match)return {child,url:match[1],token:env.ADMIN_TOKEN ?? 'fixture-admin-token'}
    if(child.exitCode!==null)throw new Error(output)
    await pause(25)
  }
  child.kill('SIGKILL');throw new Error('Startup timeout: '+output)
}
export async function stop(server){
  if(server?.child.exitCode===null&&server.child.signalCode===null){const done=once(server.child,'exit');server.child.kill('SIGTERM');await done}
}
export async function request(server,path,body){
  const response=await fetch(server.url+path,{method:body===undefined?'GET':'POST',headers:{'x-admin-token':server.token,'content-type':'application/json'},body:body===undefined?undefined:JSON.stringify(body)})
  const text=await response.text();assert.ok(response.ok,`${path}: ${response.status} ${text}`);return JSON.parse(text)
}
export async function act(server,project,plan){
  const base=`/v1/admin-agent/${project}/sessions`
  const session=await request(server,base,{})
  const previous=(await request(server,base+'/'+session.id)).events.at(-1)?.seq||0
  await request(server,base+'/'+session.id+'/messages',{content:'Execute fixture test plan. TEST_PLAN:'+JSON.stringify(plan)})
  for(let i=0;i<200;i++){
    const row=await request(server,base+'/'+session.id)
    if(row.status!=='running'){
      assert.equal(row.status,'completed',JSON.stringify(row))
      return {session:row,outputs:row.events.filter(e=>e.type==='tool_finished'&&e.seq>previous).map(e=>JSON.parse(e.output))}
    }
    await pause(25)
  }
  throw new Error('AdminAgent timeout')
}
export const policy=id=>({mode:'relay',members:[{path:[id],role:'tester'}],leader:null,rounds:1,instructions:'test'})
export async function history(server,project,id,predicate=rows=>rows.some(r=>r.type==='agent.done')){
  for(let i=0;i<250;i++){
    const rows=await request(server,`/v1/repl/${project}/sessions/${id}`)
    if(predicate(rows))return rows
    await pause(25)
  }
  throw new Error('Business history timeout')
}
