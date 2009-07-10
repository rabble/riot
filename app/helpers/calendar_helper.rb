module CalendarHelper

  def calendar_events_proc(calendar)
    lambda do |day|
      if calendar.events_on_day(day)
        [link_to(day.day, {:controller => 'calendars', :action => 'day', :calendar_id => calendar.id,:year => day.year, :month => day.month, :day => day.day}), { :class => "dayWithEvents" }]
      else
        day.day
      end
    end
  end
  
  def calendar_html(year, month, calendar, options={})
    
    later_dude_calendar = LaterDude::Calendar.new(year, month, options, &calendar_events_proc(@calendar))
    return later_dude_calendar.to_html
  end
end
