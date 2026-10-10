import {eventTime} from './chat-time.js'
// Keep the persisted event log intact; project deltas into chat messages for display.
export function conversationView(events) {
  const rows=[]
  let answer=null
  const answers=new Map(), planning=new Map(), reasoning=new Map()
  const tools=[]
  const stringify=value=>typeof value==='string'?value:JSON.stringify(value,null,2)
  for(const event of events){
    const timestamp=eventTime(event)
    const type=event.type
    const text=event.text ?? event.content ?? ''
    const speaker=event.agent||'Agent'
    const label=speaker==='default'?'默认 Agent':speaker
    const key=event.invocation_id||((event.message_id||'')+':'+speaker)
    if(event.aggregate===true)continue
    if(type==='agent.yield'){
      answers.delete(key);reasoning.delete(key)
      rows.push({seq:event.seq,type:'status',timestamp,label:'协作',text:label+' '+(text||'已让出本轮'),agent:speaker});continue
    }
    if(type==='agent.activity'){rows.push({seq:event.seq,type:'status',timestamp,label:'协作',text});continue}
    if(type==='agent.planning'){
      let row=planning.get(key)
      if(!row){row={seq:event.seq,type:'process',timestamp,label:label+' · 协作规划',text:'',agent:speaker};planning.set(key,row);rows.push(row)}
      row.text=String(text);continue
    }
    if(type==='agent.member.error'){rows.push({seq:event.seq,type:'agent.error',timestamp,label,text,agent:speaker});answers.delete(key);continue}
    if(['user','message.created','context.reset'].includes(type)){answers.clear();reasoning.clear()}
    if((type==='context_checkpoint'||type==='agent.context')&&String(text).startsWith('Context compacted:')){
      answer=null;let existing=null;for(let i=rows.length-1;i>=0;i--){if(['user','message.created','context.reset'].includes(rows[i].type))break;if(rows[i].compacted){existing=rows[i];break}}if(existing)existing.timestamp=timestamp;else rows.push({seq:event.seq,type:'status',timestamp,text:'上下文已自动压缩 · 原始日志保留',label:'上下文',compacted:true});continue
    }
    if(type==='context.reset'){answer=null;rows.push({seq:event.seq,type:'status',timestamp,text:'新上下文 · 之前的记录不再发送给模型',label:'上下文'});continue}
    if(type==='reasoning_delta'||type==='agent.reasoning'){
      let thought=event.text ?? ''
      if(!thought&&text){try{thought=JSON.parse(text).text??''}catch{thought=text}}
      if(!thought)continue
      let row=reasoning.get(key)
      if(!row){row={seq:event.seq,type:'process',timestamp,label:label+' · 思考过程',name:'reasoning',text:'',agent:speaker,invocation_id:event.invocation_id,pending:true};reasoning.set(key,row);rows.push(row)}
      row.text+=thought;answer=null;continue
    }
    if(type==='summary'){
      rows.push({seq:event.seq,type:'summary',timestamp,text,label:'摘要',compacted:true});continue
    }
    if(type==='context_checkpoint'||type==='agent.context'||type==='agent.progress')continue
    if(type==='text_delta'||type==='agent.delta'){
      if(!text)continue
      answer=answers.get(key)
      if(!answer){answer={seq:event.seq,type:'assistant',timestamp,text:'',label,agent:speaker,invocation_id:event.invocation_id};rows.push(answer);answers.set(key,answer)}
      answer.text+=text
    }else if(type==='completed'||type==='agent.done'||type==='agent.message'){
      reasoning.delete(key)
      answer=answers.get(key)
      if(text){
        if(answer)answer.text=text
        else rows.push({seq:event.seq,type:'assistant',timestamp,text,label,agent:speaker,invocation_id:event.invocation_id})
      }
      if(type==='agent.message'){answers.delete(key);continue}
      rows.push({seq:`done-${event.seq}`,type:'status',timestamp,text:'已完成',label:'状态'})
      answer=null
    }else if(type==='tool_started'||type==='agent.tool.started'){
      answer=null;answers.delete(key)
      let name=event.name
      if(!name){try{name=JSON.parse(text).name}catch{}}
      let data=event
      try{if(text)data={...event,...JSON.parse(text)}}catch{}
      const tool={seq:event.seq,type:'tool',timestamp,id:data.call_id??data.id,agent:speaker,invocation_id:event.invocation_id,name:name||data.name||'tool',text:'',input:stringify(data.input??data.arguments),label:name||data.name||'工具调用',pending:true}
      tools.push(tool);rows.push(tool)
    }else if(type==='tool_finished'||type==='agent.tool.finished'){
      let data=event
      try{if(text)data={...event,...JSON.parse(text)}}catch{}
      const id=data.call_id??data.id
      const tool=tools.find(t=>t.pending&&t.agent===speaker&&t.invocation_id===event.invocation_id&&(id?t.id===id:true))
      if(tool){
        tool.text=stringify(data.output??text);tool.pending=false
        // Older OpenCode started events omitted input; its result retains it.
        if(!tool.input||tool.input==='null'||tool.input==='{}'){
          try{const result=typeof data.output==='string'?JSON.parse(data.output):data.output;if(result?.input)tool.input=stringify(result.input)}catch{}
        }
      }
      else rows.push({seq:event.seq,type:'tool',timestamp,text:stringify(data.output??text),label:'工具结果',pending:false})
    }else{
      answer=null
      if(!['user','message.created','failed','agent.error','task.interrupted','command.result'].includes(type)){
        rows.push({seq:event.seq,type:'process',timestamp,label:type||'执行记录',text:stringify(event)});continue
      }
      rows.push({...event,timestamp,text:event.message??event.error??text,label:type==='command.result'?'群设置':['user','message.created'].includes(type)?'你':['failed','agent.error','task.interrupted'].includes(type)?'执行结束':type})
    }
  }
  return rows
}
