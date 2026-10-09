<script setup>
import {Tools,Loading,Check} from '@element-plus/icons-vue'
import {ref,computed,watch,nextTick,onBeforeUnmount} from 'vue'
import {messageClock,messageDate} from './chat-time.js'
import {processSummary,compactProcessSummary} from './process-summary.js'
import {processView} from './process-view.js'
import MessageBody from './MessageBody.vue'
const props=defineProps({messages:Array,running:Boolean,management:Boolean,emptyTitle:String,emptyDescription:String,hasMore:Boolean,loadOlder:Function,progress:String})
const emit=defineEmits(['suggest','history-error'])
const rotation=ref(0)
const rotationTimer=setInterval(()=>{if(props.running&&!document.hidden)rotation.value++},2200)
onBeforeUnmount(()=>clearInterval(rotationTimer))
const currentProcess=computed(()=>{const pending=liveRows.value.filter(d=>d.pending);const items=pending.length?pending:liveRows.value.slice(-3);return items.length?items[rotation.value%items.length]:null})
const currentSummary=computed(()=>currentProcess.value?compactProcessSummary(currentProcess.value):(props.progress||'思考中…'))
const loadingOlder=ref(false)
async function loadEarlier(){
 const el=scroller.value
 if(!el||loadingOlder.value||!props.hasMore||!props.loadOlder)return
 loadingOlder.value=true;follow.value=false
 const height=el.scrollHeight,top=el.scrollTop
 try{await props.loadOlder();await nextTick();el.scrollTop=top+el.scrollHeight-height}catch(e){emit('history-error',e.message)}finally{loadingOlder.value=false}
}
const displayRows=computed(()=>{let previous='';return processView(props.messages,props.running).map(row=>{const day=messageDate(row.timestamp);const divider=day&&day!==previous;if(day)previous=day;return {...row,day:divider?day:''}})})
const historyRows=computed(()=>displayRows.value.filter(row=>!(row.type==='process-group'&&row.pending)))
const liveRows=computed(()=>displayRows.value.filter(row=>row.type==='process-group'&&row.pending).flatMap(row=>row.details))
const activeProcess=computed(()=>displayRows.value.some(row=>row.type==='process-group'&&row.pending))
const scroller=ref(null),follow=ref(true)
function scroll(){const el=scroller.value;if(!el||loadingOlder.value)return;follow.value=el.scrollHeight-el.scrollTop-el.clientHeight<100;if(el.scrollTop<60)loadEarlier()}
watch(()=>[props.messages,props.running],async()=>{await nextTick();if(!loadingOlder.value&&follow.value&&scroller.value)scroller.value.scrollTop=scroller.value.scrollHeight},{deep:true})
</script>
<template>
  <div ref="scroller" class="messages" @scroll="scroll" @wheel.passive="event=>{if(event.deltaY<0&&scroller.scrollTop<60)loadEarlier()}">
    <div class="message-column">
      <p v-if="hasMore" class="history-status">{{loadingOlder?'正在加载更早的消息…':'向上滚动加载更早的消息'}}</p>
      <div v-if="!messages.length" class="welcome">
        <h1>{{emptyTitle||(management?'管理':'暂无消息')}}</h1>
        <p>{{emptyDescription||(management?'配置 Agent、创建项目，或查看节点与项目状态。':'在下方发送消息开始协作，交流方式可在项目配置中调整。')}}</p>
        <div v-if="management" class="suggestions"><el-button type="primary" native-type="button" @click="$emit('suggest','看看当前有哪些 Agent 和群组')">查看我的 Agent <span>↗</span></el-button><el-button type="primary" native-type="button" @click="$emit('suggest','帮我设计一个开发协作群，先给出方案')">组建一个协作群 <span>↗</span></el-button></div>
      </div>
      <template v-for="r in historyRows" :key="r.seq">
        <div v-if="r.day" class="chat-date-divider"><span>{{r.day}}</span></div>
        <div v-if="r.type==='process-group'" class="tool-wrap">
          <details class="process-line">
            <summary><el-icon><Tools/></el-icon><span>执行记录 · {{r.details.length}} 项</span></summary>
            <div v-for="detail in r.details" :key="detail.seq" class="tool-content"><b>{{processSummary(detail,false)}}</b><pre v-if="detail.input">{{detail.input}}</pre><pre>{{detail.text||'无输出'}}</pre></div>
          </details>
        </div>
        <div v-else-if="r.type==='tool'" class="tool-wrap">
          <el-collapse class="tool-collapse"><el-collapse-item :name="r.seq"><template #title><el-icon><Tools/></el-icon><span class="tool-name">{{r.label}}</span><time v-if="r.timestamp" :datetime="r.timestamp" :title="new Date(r.timestamp).toLocaleString()">{{messageClock(r.timestamp)}}</time><el-tag size="small" :type="r.pending?'warning':'info'">{{r.pending?'执行中':'已完成'}}</el-tag></template>
            <div class="tool-content"><template v-if="r.input"><b>输入</b><pre>{{r.input}}</pre></template><b>结果</b><pre>{{r.text||'等待返回…'}}</pre></div>
          </el-collapse-item></el-collapse>
        </div>
        <p v-else-if="r.type==='status'" class="turn-status" :title="r.text">{{r.text}}</p>
        <div v-else-if="r.type==='summary'" class="turn-status summary-line" :title="r.text"><span class="summary-mark">早期对话已压缩为摘要</span><span class="summary-text">{{r.text}}</span></div>
        <div v-else :class="['message-row',['user','message.created'].includes(r.type)?'from-user':'from-agent']">
          <el-avatar class="avatar" :size="32" shape="square">{{['user','message.created'].includes(r.type)?'我':(r.label||'AI').slice(0,2)}}</el-avatar>
          <div class="message-content"><span class="sender">{{r.label}}<time v-if="r.timestamp" :datetime="r.timestamp" :title="new Date(r.timestamp).toLocaleString()">{{messageClock(r.timestamp)}}</time></span><div :class="['bubble',{'failure':['failed','agent.error'].includes(r.type)}]"><MessageBody :text="r.text" /></div></div>
        </div>
      </template>

    </div>
  </div>
  <div v-if="running" class="live-process" aria-live="polite">
    <details class="process-line live-tool-status">
      <summary><el-icon :class="{'is-loading':!currentProcess||currentProcess.pending}"><Loading v-if="!currentProcess||currentProcess.pending"/><Check v-else/></el-icon><span class="rotating-status">{{currentSummary}}</span></summary>
      <div v-for="detail in liveRows" :key="detail.seq" class="tool-content"><b>{{processSummary(detail,true)}}</b><pre v-if="detail.input">{{detail.input}}</pre><pre>{{detail.text||'等待返回…'}}</pre></div>
    </details>
  </div>
</template>

<style scoped>.live-process{width:calc(100% - 56px);max-width:940px;align-self:center;max-height:160px;overflow:auto;flex-shrink:0;padding:0 16px}.live-process .thinking{margin:5px 0;font-size:13px}@media(max-width:750px){.live-process{width:calc(100% - 24px)}}</style>
<style scoped>.messages{overflow-anchor:none}.history-status{text-align:center;color:var(--muted);font-size:12px;margin:0 0 14px} .process-line{color:var(--muted);font-size:13px;margin:5px 0}.process-line summary{display:flex;align-items:center;gap:8px;cursor:pointer;list-style:none;padding:4px 0;min-width:0}.process-line summary::-webkit-details-marker{display:none}.process-line summary span{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;min-width:0}.process-line[open] summary{color:var(--text)}.process-line .el-icon{flex-shrink:0}.process-line .tool-content{margin:4px 0 6px 18px;padding:4px 8px;border-left:1px solid var(--line)}.tool-content{padding:8px 10px;line-height:1.4}.tool-content b{display:block;font-size:11px;line-height:1.4;font-weight:500}.tool-content pre{margin:4px 0 6px;padding:6px 8px;font:12px/1.4 ui-monospace,SFMono-Regular,Consolas,monospace;white-space:pre;overflow-wrap:normal;tab-size:2;max-height:240px;overflow:auto;letter-spacing:normal}.tool-content pre:last-child{margin-bottom:0}</style>

<style scoped>.execution-status{flex-shrink:0;padding:8px 24px;color:var(--muted);font-size:12px;border-top:1px solid var(--line)}</style>

<style scoped>.rotating-status{display:block;min-width:0;flex:1;min-height:20px;line-height:20px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.turn-status{white-space:nowrap;overflow:hidden;text-overflow:ellipsis;line-height:20px;margin-block:4px}.summary-line{display:flex;gap:8px;color:var(--muted);padding:6px 10px;background:color-mix(in srgb,var(--line) 30%,transparent);border-radius:8px;margin-block:8px}.summary-mark{flex-shrink:0;font-size:12px}.summary-text{font-size:12px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}</style>
