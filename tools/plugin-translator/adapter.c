// Мост TeaVM -> WIT; память принадлежит одному компоненту и ограничивается хозяином.
#include "plugin.h"
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <stdint.h>
extern int teavm_main(int argc, char **argv);
extern int32_t mcsa_dispatch(int32_t kind, void *data, int32_t len);
static char *output, *event_text;
static bool enabled;
static plugin_list_string_t completions;
static size_t output_len, output_cap;
static void clear(void) { output_len=0; }
static void bytes(const void *p,size_t len) {
    if (output_len+len+1>4*1024*1024) abort();
    if (output_len+len+1>output_cap) { output_cap=(output_len+len+1)*2; output=realloc(output,output_cap); if(!output)abort(); }
    if(len)memcpy(output+output_len,p,len); output_len+=len; output[output_len++]=0;
}
static void text(const char *s) { bytes(s,strlen(s)); }
static void string(plugin_string_t s) { bytes(s.ptr,s.len); }
static void number(double n) { char s[64]; snprintf(s,sizeof(s),"%.17g",n); text(s); }
static plugin_string_t str(const char *s) { return (plugin_string_t){(uint8_t*)s,strlen(s)}; }
static void error(plugin_string_t s) { text("!"); string(s); plugin_string_free(&s); }
static void player(mcsa_api_types_player_t *p) {
    string(p->uuid);string(p->name);number(p->game_mode);number(p->dimension);
    number(p->x);number(p->y);number(p->z);number(p->yaw);number(p->pitch);
}
int32_t mcsa_length(void) { return (int32_t)output_len; }
void *mcsa_call(int32_t op,void *data,int32_t len) {
    char *a[16]={0}; int count=0; char *copy=malloc((size_t)len+1); if(!copy)abort();
    memcpy(copy,data,len);copy[len]=0;
    for(int i=0;i<len&&count<16;){a[count++]=copy+i;i+=(int)strlen(copy+i)+1;}
    plugin_string_t s[16];for(int i=0;i<16;i++)s[i]=str(a[i]?a[i]:"");
    plugin_string_t err={0};clear();
    switch(op) {
    case 0: mcsa_api_server_broadcast(&s[0]);break;
    case 1: {mcsa_api_server_list_player_t list;mcsa_api_server_online_players(&list);for(size_t i=0;i<list.len;i++)player(&list.ptr[i]);mcsa_api_server_list_player_free(&list);break;}
    case 2: number(mcsa_api_player_send_message(&s[0],&s[1]));break;
    case 3: {mcsa_api_types_location_t loc={.dimension=atoi(a[1]),.x=atof(a[2]),.y=atof(a[3]),.z=atof(a[4]),.yaw={true,atof(a[5])},.pitch={true,atof(a[6])}};number(mcsa_api_player_teleport(&s[0],&loc));break;}
    case 4: {mcsa_api_command_command_spec_t spec={.name=s[0],.description=s[1],.usage=s[2],.level=atoi(a[3])};if(!mcsa_api_command_register(&spec,&err))error(err);break;}
    case 5: number(mcsa_api_scheduler_run_later(atoi(a[0]),atoi(a[2])));break;
    case 6: number(mcsa_api_scheduler_run_repeating(atoi(a[0]),atoi(a[1]),atoi(a[2])));break;
    case 7: number(mcsa_api_scheduler_cancel(atoi(a[0])));break;
    case 8: {plugin_option_list_u8_t value;if(!mcsa_api_storage_read(&s[0],&value,&err))error(err);else {if(value.is_some)bytes(value.val.ptr,value.val.len);else text("");plugin_option_list_u8_free(&value);}break;}
    case 26: {plugin_option_list_u8_t value;if(!mcsa_api_storage_read(&s[0],&value,&err))error(err);else {if(!value.is_some){plugin_list_u8_t initial={s[1].ptr,s[1].len};if(!mcsa_api_storage_write(&s[0],&initial,&err))error(err);}plugin_option_list_u8_free(&value);}break;}
    case 9: {plugin_list_u8_t value={s[1].ptr,s[1].len};if(!mcsa_api_storage_write(&s[0],&value,&err))error(err);break;}
    case 10: {uint8_t level=0;mcsa_api_player_operator_level(&s[0],&level);number(level);break;}
    case 11: number(mcsa_api_player_set_game_mode(&s[0],atoi(a[1])));break;
    case 12: {mcsa_api_presentation_player_info_t info;if(mcsa_api_presentation_get_player_info(&s[0],&info)){number(info.health);mcsa_api_presentation_player_info_free(&info);}else {text("!");text("Игрок отсутствует");}break;}
    case 13:case 14: {mcsa_api_types_block_pos_t pos={atoi(a[0]),atoi(a[1]),atoi(a[2]),atoi(a[3])};if(op==13){plugin_string_t block;if(mcsa_api_world_get_block(&pos,&block)){string(block);plugin_string_free(&block);}else {text("!");text("Чанк не загружен");}}else if(!mcsa_api_world_set_block(&pos,&s[4],&err))error(err);break;}
    case 15: {mcsa_api_types_location_t loc;mcsa_api_world_spawn_location(atoi(a[0]),&loc);number(loc.x);number(loc.y);number(loc.z);break;}
    case 16: number(mcsa_api_world_time(atoi(a[0])));break;
    case 17: mcsa_api_world_set_time(atoi(a[0]),atoi(a[1]));break;
    case 18: {plugin_string_t weather;mcsa_api_world_weather(atoi(a[0]),&weather);string(weather);plugin_string_free(&weather);break;}
    case 19: if(!mcsa_api_world_set_weather(atoi(a[0]),&s[1],12000,&err))error(err);break;
    case 20: number(mcsa_api_player_kick(&s[0],&s[1]));break;
    case 21: {mcsa_api_types_item_stack_t item={s[2],atoi(a[3])};if(!mcsa_api_player_set_inventory_slot(&s[0],atoi(a[1]),item.count?&item:NULL,&err))error(err);break;}
    case 22: free(event_text);event_text=strdup(a[0]);break;
    case 23: mcsa_api_log_write(MCSA_API_LOG_LEVEL_INFO,&s[0]);break;
    case 24: enabled=true;break;
    case 25: if(completions.len<100 && s[0].len<=512) { completions.ptr=realloc(completions.ptr,(completions.len+1)*sizeof(plugin_string_t));if(!completions.ptr)abort();plugin_string_dup(&completions.ptr[completions.len++],a[0]); }break;
    case 27: number(mcsa_api_presentation_send_component(&s[0],&s[1]));break;
    case 28: number(mcsa_api_presentation_set_action_bar(&s[0],&s[1]));break;
    case 29: if(!mcsa_api_player_set_health(&s[0],atof(a[1]),&err))error(err);break;
    case 30: {mcsa_api_player_potion_effect_t effect={.kind=s[1],.duration=atoi(a[2]),.amplifier=atoi(a[3]),.ambient=!strcmp(a[4],"true"),.particles=!strcmp(a[5],"true"),.icon=!strcmp(a[6],"true")};if(!mcsa_api_player_add_potion_effect(&s[0],&effect,&err))error(err);else number(1);break;}
    case 31: if(!mcsa_api_player_remove_potion_effect(&s[0],&s[1],&err))error(err);break;
    case 32: {mcsa_api_world_border_t border;mcsa_api_world_get_border(atoi(a[0]),&border);number(border.size);number(border.center_x);number(border.center_z);break;}
    case 33: {mcsa_api_world_border_t border={atof(a[1]),atof(a[2]),atof(a[3])};if(!mcsa_api_world_set_border(atoi(a[0]),&border,&err))error(err);break;}
    default: abort();
    }
    free(copy);return output;
}
static int dispatch(int kind) {
    size_t len=output_len;char *input=malloc(len?len:1);if(!input)abort();if(len)memcpy(input,output,len);
    free(event_text);event_text=NULL;int result=mcsa_dispatch(kind,input,(int32_t)len);free(input);return result;
}
void exports_mcsa_api_lifecycle_enable(void) { char *args[]={"plugin",NULL};teavm_main(1,args);if(!enabled)abort(); }
void exports_mcsa_api_lifecycle_disable(void) { clear();dispatch(0);free(event_text);event_text=NULL;free(output);output=NULL;output_len=output_cap=0; }
void exports_mcsa_api_events_on_player_join(mcsa_api_types_player_t *who) {clear();player(who);dispatch(1);}
void exports_mcsa_api_events_on_player_quit(mcsa_api_types_player_t *who) {clear();player(who);dispatch(2);}
void exports_mcsa_api_events_on_chat(mcsa_api_types_player_t *who,plugin_string_t *message,mcsa_api_types_chat_result_t *ret) {
    clear();player(who);string(*message);int result=dispatch(3);ret->tag=result;
    if(result==2)plugin_string_dup(&ret->val.replace,event_text?event_text:"");
}
static uint8_t block_event(int kind,mcsa_api_types_player_t *who,mcsa_api_types_block_pos_t *pos,plugin_string_t *block) {
    clear();player(who);number(pos->dimension);number(pos->x);number(pos->y);number(pos->z);string(*block);return dispatch(kind)?1:0;
}
uint8_t exports_mcsa_api_events_on_block_break(mcsa_api_types_player_t *who,mcsa_api_types_block_pos_t *pos,plugin_string_t *block) {return block_event(4,who,pos,block);}
uint8_t exports_mcsa_api_events_on_block_place(mcsa_api_types_player_t *who,mcsa_api_types_block_pos_t *pos,plugin_string_t *block) {return block_event(5,who,pos,block);}
uint8_t exports_mcsa_api_events_on_player_command(mcsa_api_types_player_t *who,plugin_string_t *line) {clear();player(who);string(*line);return dispatch(6)?1:0;}
void exports_mcsa_api_events_on_command(mcsa_api_types_sender_t *caller,plugin_string_t *name,plugin_list_string_t *args,mcsa_api_types_command_result_t *ret) {
    clear();number(caller->kind);if(caller->uuid.is_some)string(caller->uuid.val);else text("");string(caller->name);string(*name);
    for(size_t i=0;i<args->len;i++)string(args->ptr[i]);ret->success=dispatch(7)!=0;ret->message.is_some=false;
}
void exports_mcsa_api_events_on_task(uint32_t task) {clear();number(task);dispatch(8);}

bool exports_mcsa_api_combat_events_on_damage(exports_mcsa_api_combat_events_player_t *who,plugin_string_t *cause,double damage,double *ret) {
    clear();player(who);string(*cause);number(damage);int result=dispatch(10);
    if(result==1)return false;
    *ret=result==2&&event_text?atof(event_text):damage;return true;
}
void exports_mcsa_api_combat_events_on_death(exports_mcsa_api_combat_events_player_t *who,bool keep_inventory) {
    clear();player(who);number(keep_inventory);dispatch(11);
}

void exports_mcsa_api_completion_on_tab_complete(mcsa_api_types_sender_t *caller,plugin_string_t *name,plugin_list_string_t *args,plugin_list_string_t *ret) {
    clear();number(caller->kind);if(caller->uuid.is_some)string(caller->uuid.val);else text("");string(caller->name);string(*name);
    for(size_t i=0;i<args->len;i++)string(args->ptr[i]);
    completions=(plugin_list_string_t){0};dispatch(9);*ret=completions;completions=(plugin_list_string_t){0};
}
