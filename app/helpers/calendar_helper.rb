module CalendarHelper

  def calendar_events_proc(calendar)
    lambda do |day|
      if calendar.events_on_day(day)
        [link_to(day.day, events_path(day.year, day.month, day.day)), { :class => "dayWithEvents" }]
      else
        day.day
      end
    end
  end

end
