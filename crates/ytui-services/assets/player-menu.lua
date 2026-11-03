-- ytui's mpv window menu. Built-in OSC remains available with the mouse.
local mp = require 'mp'
local msg = require 'mp.msg'
local assdraw = require 'mp.assdraw'
local visible, selected = false, 1
local entries = {
  {'Play / Pause', function() mp.commandv('cycle','pause') end},
  {'Quality: best', function() mp.set_property('user-data/ytui/action','quality:best') end},
  {'Quality: 1080p', function() mp.set_property('user-data/ytui/action','quality:1080') end},
  {'Quality: 720p', function() mp.set_property('user-data/ytui/action','quality:720') end},
  {'Quality: 480p', function() mp.set_property('user-data/ytui/action','quality:480') end},
  {'Audio track: next', function() mp.commandv('cycle','aid') end},
  {'Subtitle track: next', function() mp.commandv('cycle','sid') end},
  {'Subtitles: show / hide', function() mp.commandv('cycle','sub-visibility') end},
  {'Speed: slower', function() mp.set_property_number('speed',math.max(0.25,mp.get_property_number('speed',1)-0.25)) end},
  {'Speed: faster', function() mp.set_property_number('speed',math.min(4,mp.get_property_number('speed',1)+0.25)) end},
  {'Previous chapter', function() mp.commandv('add','chapter',-1) end},
  {'Next chapter', function() mp.commandv('add','chapter',1) end},
  {'SponsorBlock: toggle skipping', function() mp.set_property('user-data/ytui/action','sponsor-toggle') end},
  {'SponsorBlock: replay skipped part', function() mp.set_property('user-data/ytui/action','sponsor-undo') end},
  {'Fullscreen', function() mp.commandv('cycle','fullscreen') end},
}
local overlay = mp.create_osd_overlay('ass-events')
local function render()
  if not visible then overlay:remove(); return end
  local _,height = mp.get_osd_size()
  local size = math.max(13,math.min(25,math.floor(height/35)))
  local a = assdraw.ass_new()
  a:append(string.format('{\\an7\\pos(32,28)\\fs%d\\bord2\\shad1\\1c&HFFFFFF&\\3c&H101010&}ytui  |  Player settings\\N',size+4))
  a:append(string.format('{\\fs%d}Up/Down  choose    Enter  apply    Esc/G  close\\N\\N',size-3))
  for i,entry in ipairs(entries) do
    a:append(string.format('{\\fs%d\\1c&H%s&}%s %s\\N',size,i==selected and 'B8E5A6' or 'DDDDDD',i==selected and '>' or ' ',entry[1]))
  end
  overlay.data=a.text; overlay:update()
end
local function close()
  visible=false
  for _,name in ipairs({'up','down','enter','esc'}) do mp.remove_key_binding('ytui-menu-'..name) end
  render()
end
local function toggle()
  if visible then close();return end
  visible=true
  mp.add_forced_key_binding('UP','ytui-menu-up',function() selected=math.max(1,selected-1);render() end,{repeatable=true})
  mp.add_forced_key_binding('DOWN','ytui-menu-down',function() selected=math.min(#entries,selected+1);render() end,{repeatable=true})
  mp.add_forced_key_binding('ENTER','ytui-menu-enter',function() entries[selected][2]();render() end)
  mp.add_forced_key_binding('ESC','ytui-menu-esc',close)
  render()
end
mp.add_key_binding('g','ytui-player-menu',toggle)
mp.register_script_message('ytui-menu',toggle)
-- SponsorBlock marks: the OSC seekbar draws a tick per chapter, so each
-- segment's start/end becomes a chapter merged with the video's own chapters.
local base_chapters, marks = nil, {}
local function mark_label(category)
  local text = tostring(category or 'segment'):gsub('_',' ')
  return 'SponsorBlock: '..text:sub(1,1):upper()..text:sub(2)
end
local function apply_marks()
  if base_chapters == nil then return end
  local duration = mp.get_property_number('duration')
  local list = {}
  for _,chapter in ipairs(base_chapters) do list[#list+1] = {time=chapter.time,title=chapter.title} end
  for _,segment in ipairs(marks) do
    local start, stop = tonumber(type(segment)=='table' and segment.start), tonumber(type(segment)=='table' and segment['end'])
    if start and stop and stop > start then
      list[#list+1] = {time=math.max(0,start),title=mark_label(segment.category)}
      if not duration or stop < duration - 0.5 then
        -- After the mark the timeline resumes the enclosing original chapter.
        local resume = ''
        for _,chapter in ipairs(base_chapters) do
          if chapter.time <= stop then resume = chapter.title or '' end
        end
        list[#list+1] = {time=stop,title=resume}
      end
    end
  end
  table.sort(list,function(a,b) return a.time < b.time end)
  mp.set_property_native('chapter-list',list)
end
mp.observe_property('user-data/ytui/segments','native',function(_,value)
  marks = type(value)=='table' and value or {}
  apply_marks()
end)
mp.register_event('file-loaded',function()
  base_chapters = mp.get_property_native('chapter-list') or {}
  apply_marks()
  mp.osd_message('ytui | G: player settings | mouse: playback controls',4)
end)
-- A successful process exit also occurs when a window is closed. Report actual
-- EOF separately so the terminal queue cannot advance on a user quit.
mp.register_event('end-file',function(event)
  base_chapters = nil
  if event.reason == 'eof' then msg.warn('YTUI_END_FILE:eof') end
end)
mp.observe_property('osd-dimensions','native',render)
