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
    if (output_len+len+1>4*1024*1024+1024) abort();
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
static void hex_bytes(const uint8_t *data,size_t len) {char *s=malloc(len*2+1);if(!s)abort();for(size_t i=0;i<len;i++){s[i*2]="0123456789abcdef"[data[i]>>4];s[i*2+1]="0123456789abcdef"[data[i]&15];}s[len*2]=0;text(s);free(s);}
static int hex_digit(char c) {return c>='0'&&c<='9'?c-'0':c>='a'&&c<='f'?c-'a'+10:-1;}
static bool utf8_text(const uint8_t *data,size_t len) {
    for(size_t i=0;i<len;i++){uint8_t c=data[i];if(!c)return false;if(c<128)continue;
        size_t rest;uint8_t low=128,high=191;
        if(c>=194&&c<=223)rest=1;else if(c>=224&&c<=239){rest=2;if(c==224)low=160;if(c==237)high=159;}else if(c>=240&&c<=244){rest=3;if(c==240)low=144;if(c==244)high=143;}else return false;
        if(i+rest>=len||data[i+1]<low||data[i+1]>high)return false;
        for(size_t j=2;j<=rest;j++)if(data[i+j]<128||data[i+j]>191)return false;i+=rest;
    }return true;
}
int32_t mcsa_length(void) { return (int32_t)output_len; }
void *mcsa_call(int32_t op,void *data,int32_t len) {
    char *a[1030]={0}; int count=0; char *copy=malloc((size_t)len+1); if(!copy)abort();
    memcpy(copy,data,len);copy[len]=0;
    for(int i=0;i<len&&count<1030;){a[count++]=copy+i;i+=(int)strlen(copy+i)+1;}
    plugin_string_t s[1030];for(int i=0;i<(count>16?count:16);i++)s[i]=str(a[i]?a[i]:"");
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
    case 8: {plugin_option_list_u8_t value;if(!mcsa_api_storage_read(&s[0],&value,&err))error(err);else {if(value.is_some){if(!utf8_text(value.val.ptr,value.val.len)){text("!");text("Файл не является текстом UTF-8 без NUL");}else bytes(value.val.ptr,value.val.len);}else text("");plugin_option_list_u8_free(&value);}break;}
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
    case 34: {plugin_option_list_u8_t value;if(!mcsa_api_storage_read(&s[0],&value,&err))error(err);else {number(value.is_some);plugin_option_list_u8_free(&value);}break;}
    case 35: {bool removed;if(!mcsa_api_storage_delete(&s[0],&removed,&err))error(err);else number(removed);break;}
    case 36: if(!mcsa_api_storage_transfer(&s[0],&s[1],!strcmp(a[2],"true"),!strcmp(a[3],"true"),&err))error(err);break;
    case 37: {mcsa_api_inventory_list_option_item_t items;if(!mcsa_api_inventory_snapshot(&s[0],&items,&err))error(err);else {for(size_t i=0;i<items.len;i++){if(items.ptr[i].is_some){string(items.ptr[i].val.kind);number(items.ptr[i].val.count);string(items.ptr[i].val.metadata);}else {text("air");text("0");text("");}}mcsa_api_inventory_list_option_item_free(&items);}break;}
    case 38: {mcsa_api_inventory_item_t item={.kind=s[2],.count=atoi(a[3]),.metadata=s[4]};if(!mcsa_api_inventory_set(&s[0],atoi(a[1]),item.count?&item:NULL,&err))error(err);break;}
    case 39: {uint32_t selected;if(!mcsa_api_inventory_selected(&s[0],&selected,&err))error(err);else number(selected);break;}
    case 40: {size_t n=s[2].len;if(n>65536||n%2){text("!");text("HTTPS: неверное тело");break;}plugin_list_u8_t body={.ptr=malloc(n/2?n/2:1),.len=n/2};if(!body.ptr)abort();bool valid=true;for(size_t i=0;i<body.len;i++){int hi=hex_digit(a[2][i*2]),lo=hex_digit(a[2][i*2+1]);if(hi<0||lo<0){valid=false;break;}body.ptr[i]=(uint8_t)((hi<<4)|lo);}uint32_t handle;if(!valid){text("!");text("HTTPS: неверное тело");}else if(!mcsa_api_https_start(&s[0],&s[1],&body,&handle,&err))error(err);else number(handle);free(body.ptr);break;}
    case 41: {mcsa_api_https_option_response_t response;if(!mcsa_api_https_poll(strtoul(a[0],NULL,10),&response,&err))error(err);else {if(response.is_some){number(response.val.status);hex_bytes(response.val.body.ptr,response.val.body.len);}mcsa_api_https_option_response_free(&response);}break;}
    case 42: {mcsa_api_attributes_snapshot_t value;if(!mcsa_api_attributes_get(&s[0],&s[1],&value,&err))error(err);else {number(value.base);number(value.value);for(size_t i=0;i<value.modifiers.len;i++){string(value.modifiers.ptr[i].id);number(value.modifiers.ptr[i].amount);number(value.modifiers.ptr[i].operation);}mcsa_api_attributes_snapshot_free(&value);}break;}
    case 43: if(!mcsa_api_attributes_set_base(&s[0],&s[1],atof(a[2]),&err))error(err);break;
    case 44: {mcsa_api_attributes_modifier_t modifier={.id=s[2],.amount=atof(a[3]),.operation=atoi(a[4])};if(!mcsa_api_attributes_add(&s[0],&s[1],&modifier,&err))error(err);break;}
    case 45: if(!mcsa_api_attributes_remove(&s[0],&s[1],&s[2],&err))error(err);break;
    case 50: {uint32_t handle;if(!mcsa_api_inventory_chest_create(atoi(a[0]),&s[1],&handle,&err))error(err);else number(handle);break;}
    case 51: {mcsa_api_inventory_item_t item={.kind=s[2],.count=atoi(a[3]),.metadata=s[4]};if(!mcsa_api_inventory_chest_set(atoi(a[0]),atoi(a[1]),item.count?&item:NULL,&err))error(err);break;}
    case 52: {mcsa_api_inventory_list_option_item_t items;if(!mcsa_api_inventory_chest_snapshot(atoi(a[0]),&items,&err))error(err);else {for(size_t i=0;i<items.len;i++){if(items.ptr[i].is_some){string(items.ptr[i].val.kind);number(items.ptr[i].val.count);string(items.ptr[i].val.metadata);}else {text("air");text("0");text("");}}mcsa_api_inventory_list_option_item_free(&items);}break;}
    case 53: if(!mcsa_api_inventory_chest_open(&s[0],atoi(a[1]),&err))error(err);break;
    case 54: if(!mcsa_api_inventory_chest_close(&s[0],&err))error(err);break;
    case 55: {uint32_t handle;if(mcsa_api_inventory_chest_current(&s[0],&handle))number(handle);break;}
    case 56: if(!mcsa_api_inventory_chest_release(atoi(a[0]),&err))error(err);break;
    case 57: {mcsa_api_inventory_option_item_t item;if(!mcsa_api_inventory_cursor(&s[0],&item,&err))error(err);else {if(item.is_some){string(item.val.kind);number(item.val.count);string(item.val.metadata);}mcsa_api_inventory_option_item_free(&item);}break;}
    case 58: {mcsa_api_presentation_team_t team={.id=s[1],.prefix=s[2],.suffix=s[3],.color=s[4],.visibility=s[5],.members={.ptr=s+6,.len=count-6}};number(mcsa_api_presentation_set_team(&s[0],&team));break;}
    case 59: number(mcsa_api_presentation_remove_team(&s[0],&s[1]));break;
    case 60: number(mcsa_api_server_current_tick());break;
    case 61: {mcsa_api_inventory_view_t view;if(!mcsa_api_inventory_open_view(&s[0],&view,&err))error(err);else {string(view.kind);for(size_t i=0;i<view.items.len;i++){if(view.items.ptr[i].is_some){string(view.items.ptr[i].val.kind);number(view.items.ptr[i].val.count);string(view.items.ptr[i].val.metadata);}else {text("air");text("0");text("");}}mcsa_api_inventory_view_free(&view);}break;}
    case 62: {mcsa_api_presentation_player_info_t info;if(mcsa_api_presentation_get_player_info(&s[0],&info)){number(info.food);number(info.level);number(info.exp);mcsa_api_presentation_player_info_free(&info);}break;}
    case 63: {mcsa_api_types_block_pos_t pos={atoi(a[1]),atoi(a[2]),atoi(a[3]),atoi(a[4])};mcsa_api_player_bed_action_t action;if(mcsa_api_player_get_bed_action(&s[0],&pos,&action)){number(action.bed.dimension);number(action.bed.x);number(action.bed.y);number(action.bed.z);number(action.problem);number(action.can_spawn);number(action.can_sleep);}break;}
    case 64: {mcsa_api_types_location_t to={.dimension=atoi(a[1]),.x=atof(a[2]),.y=atof(a[3]),.z=atof(a[4]),.yaw={true,atof(a[5])},.pitch={true,0}};if(!mcsa_api_player_set_respawn(&s[0],!strcmp(a[6],"1")?&to:NULL,&err))error(err);break;}
    case 65: {mcsa_api_player_state_t state;if(!mcsa_api_player_get_state(&s[0],&state,&err))error(err);else {number(state.fire_ticks);number(state.dead);number(state.sneaking);number(state.on_ground);number(state.in_water);number(state.in_lava);number(state.in_rain);number(state.eye_height);number(state.using_item);number(state.off_hand);for(size_t i=0;i<state.effects.len;i++){mcsa_api_player_potion_effect_t e=state.effects.ptr[i];string(e.kind);number(e.duration);number(e.amplifier);number(e.ambient);number(e.particles);number(e.icon);}mcsa_api_player_state_free(&state);}break;}
    case 66: if(!mcsa_api_player_set_fire_ticks(&s[0],atoi(a[1]),&err))error(err);break;
    case 67: if(!mcsa_api_player_damage(&s[0],atof(a[1]),&err))error(err);break;
    case 68: if(!mcsa_api_player_give_experience(&s[0],atoi(a[1]),&err))error(err);break;
    case 69: {mcsa_api_types_block_pos_t pos={atoi(a[0]),atoi(a[1]),atoi(a[2]),atoi(a[3])};mcsa_api_world_block_state_t state;if(!mcsa_api_world_block_details(&pos,&state,&err))error(err);else {string(state.data);number(state.sky);number(state.light);number(state.liquid);number(state.passable);number(state.age);number(state.max_age);mcsa_api_world_block_state_free(&state);}break;}
    case 70: {mcsa_api_types_block_pos_t pos={atoi(a[0]),atoi(a[1]),atoi(a[2]),atoi(a[3])};bool result;if(!mcsa_api_world_update_block(&pos,&s[4],!strcmp(a[5],"true"),!strcmp(a[6],"true"),&result,&err))error(err);else number(result);break;}
    case 71: {mcsa_api_types_block_pos_t pos={atoi(a[0]),atoi(a[1]),atoi(a[2]),atoi(a[3])};if(!mcsa_api_world_random_tick(&pos,&err))error(err);break;}
    case 72: number(mcsa_api_world_chunk_loaded(atoi(a[0]),atoi(a[1]),atoi(a[2])));break;
    case 73: {int32_t y;if(!mcsa_api_world_highest_block(atoi(a[0]),atoi(a[1]),atoi(a[2]),&y,&err))error(err);else number(y);break;}
    case 74: number(mcsa_api_world_random_tick_speed());break;
    case 75: {mcsa_api_types_location_t at={.dimension=atoi(a[0]),.x=atof(a[1]),.y=atof(a[2]),.z=atof(a[3])};if(!mcsa_api_world_play_sound(&at,&s[4],atof(a[5]),atof(a[6]),&err))error(err);break;}
    case 76: {mcsa_api_entities_vector_t v;if(!mcsa_api_entities_velocity(&s[0],&v,&err))error(err);else {number(v.x);number(v.y);number(v.z);}break;}
    case 77: {mcsa_api_entities_vector_t v={atof(a[1]),atof(a[2]),atof(a[3])};if(!mcsa_api_entities_set_velocity(&s[0],&v,&err))error(err);break;}
    case 78: if(!mcsa_api_entities_remove(&s[0],&err))error(err);break;
    case 79: {mcsa_api_types_location_t at={.dimension=atoi(a[0]),.x=atof(a[1]),.y=atof(a[2]),.z=atof(a[3])};mcsa_api_inventory_item_t item={.kind=s[4],.count=atoi(a[5]),.metadata=s[6]};plugin_string_t uuid;if(!mcsa_api_entities_drop_item(&at,&item,!strcmp(a[7],"true"),&uuid,&err))error(err);else {string(uuid);plugin_string_free(&uuid);}break;}
    case 80: {mcsa_api_inventory_item_t item;if(!mcsa_api_entities_item_stack(&s[0],&item,&err))error(err);else {string(item.kind);number(item.count);string(item.metadata);mcsa_api_inventory_item_free(&item);}break;}
    case 81: {mcsa_api_inventory_item_t item={.kind=s[1],.count=atoi(a[2]),.metadata=s[3]};if(!mcsa_api_entities_set_item_stack(&s[0],&item,&err))error(err);break;}
    case 82: {mcsa_api_types_location_t at;if(!mcsa_api_entities_item_location(&s[0],&at,&err))error(err);else {number(at.dimension);number(at.x);number(at.y);number(at.z);}break;}
    case 83: {mcsa_api_types_location_t at={.dimension=atoi(a[1]),.x=atof(a[2]),.y=atof(a[3]),.z=atof(a[4])};bool result;if(!mcsa_api_entities_item_teleport(&s[0],&at,&result,&err))error(err);else number(result);break;}
    case 84: number(mcsa_api_entities_valid(&s[0]));break;
    case 86: {mcsa_api_types_location_t at={.dimension=atoi(a[0]),.x=atof(a[1]),.y=atof(a[2]),.z=atof(a[3])};if(!mcsa_api_world_spawn_particle(&at,&s[4],atoi(a[5]),atof(a[6]),atof(a[7]),atof(a[8]),&err))error(err);break;}
    case 87: {mcsa_api_types_block_pos_t pos={atoi(a[0]),atoi(a[1]),atoi(a[2]),atoi(a[3])};bool result;if(!mcsa_api_world_break_naturally(&pos,&result,&err))error(err);else number(result);break;}
    case 88: number(mcsa_api_server_is_stopping());break;
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
bool exports_mcsa_api_combat_events_on_damage_from(exports_mcsa_api_combat_events_player_t *who,exports_mcsa_api_combat_events_player_t *attacker,plugin_string_t *cause,double damage,double *ret) {
    clear();player(who);string(*cause);number(damage);number(attacker!=NULL);if(attacker)player(attacker);int result=dispatch(18);
    if(result==1)return false;*ret=result==2&&event_text?atof(event_text):damage;return true;
}
void exports_mcsa_api_combat_events_on_death(exports_mcsa_api_combat_events_player_t *who,bool keep_inventory) {
    clear();player(who);number(keep_inventory);dispatch(11);
}

void exports_mcsa_api_completion_on_tab_complete(mcsa_api_types_sender_t *caller,plugin_string_t *name,plugin_list_string_t *args,plugin_list_string_t *ret) {
    clear();number(caller->kind);if(caller->uuid.is_some)string(caller->uuid.val);else text("");string(caller->name);string(*name);
    for(size_t i=0;i<args->len;i++)string(args->ptr[i]);
    completions=(plugin_list_string_t){0};dispatch(9);*ret=completions;completions=(plugin_list_string_t){0};
}

bool exports_mcsa_api_inventory_events_on_inventory(mcsa_api_types_player_t *who,uint32_t handle,uint32_t kind,int32_t slot,int32_t button,int32_t mode) {
    clear();player(who);number(handle);number(kind);number(slot);number(button);number(mode);return dispatch(12)!=0;
}

bool exports_mcsa_api_item_events_on_item(mcsa_api_types_player_t *who,uint32_t kind,int32_t slot) {
    clear();player(who);number(kind);number(slot);return dispatch(16)!=0;
}

bool exports_mcsa_api_item_events_on_drag(mcsa_api_types_player_t *who,uint32_t handle,plugin_list_s32_t *slots) {
    clear();player(who);number(handle);for(size_t i=0;i<slots->len;i++)number(slots->ptr[i]);return dispatch(17)!=0;
}

bool exports_mcsa_api_player_events_on_move(mcsa_api_types_player_t *who,mcsa_api_types_location_t *destination,bool jump,mcsa_api_types_location_t *ret) {
    clear();player(who);number(destination->dimension);number(destination->x);number(destination->y);number(destination->z);number(destination->yaw.is_some?destination->yaw.val:who->yaw);number(destination->pitch.is_some?destination->pitch.val:who->pitch);number(jump);
    int result=dispatch(13);if(result==1)return false;*ret=*destination;
    if(result==2&&event_text){unsigned dimension;double x,y,z;float yaw,pitch;if(sscanf(event_text,"%u %lf %lf %lf %f %f",&dimension,&x,&y,&z,&yaw,&pitch)!=6||dimension>2)abort();ret->dimension=dimension;ret->x=x;ret->y=y;ret->z=z;ret->yaw.is_some=ret->pitch.is_some=true;ret->yaw.val=yaw;ret->pitch.val=pitch;}return true;
}
uint32_t exports_mcsa_api_player_events_on_interact(mcsa_api_types_player_t *who,uint32_t action,uint32_t hand,mcsa_api_types_block_pos_t *block,uint32_t face) {
    clear();player(who);number(action);number(hand);number(block!=NULL);number(block?block->dimension:who->dimension);number(block?block->x:0);number(block?block->y:0);number(block?block->z:0);number(face);return (uint32_t)dispatch(14);
}
void exports_mcsa_api_player_events_on_respawn(mcsa_api_types_player_t *who,mcsa_api_types_location_t *destination,bool bed,bool anchor,mcsa_api_types_location_t *ret) {
    clear();player(who);number(destination->dimension);number(destination->x);number(destination->y);number(destination->z);number(destination->yaw.is_some?destination->yaw.val:who->yaw);number(destination->pitch.is_some?destination->pitch.val:who->pitch);number(bed);number(anchor);
    dispatch(15);*ret=*destination;unsigned dimension;double x,y,z;float yaw,pitch;if(!event_text||sscanf(event_text,"%u %lf %lf %lf %f %f",&dimension,&x,&y,&z,&yaw,&pitch)!=6||dimension>2)abort();ret->dimension=dimension;ret->x=x;ret->y=y;ret->z=z;ret->yaw.is_some=ret->pitch.is_some=true;ret->yaw.val=yaw;ret->pitch.val=pitch;
}
