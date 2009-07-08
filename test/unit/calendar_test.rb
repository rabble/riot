require File.dirname(__FILE__) + '/../test_helper'


class CalendarTest < ActiveSupport::TestCase

  context "event scopes" do
    
    test "events on scope by day" do
      calendar = Calendar.make
      event_on_day = Event.make(:calendar => calendar, :starts_at => Time.parse( 'Thu Jan 01 00:00:01 UTC 1970'), :ends_at => Time.parse('Thu Jan 01 00:01:00 UTC 1970'))
      event_later = Event.make(:calendar => calendar, :starts_at => Time.parse( 'Thu Jan 02 00:00:01 UTC 1970'), :ends_at => Time.parse('Thu Jan 02 00:01:00 UTC 1970'))

      events_on_day = calendar.events_on_day(Date.civil(1970,1,1))
      events_on_day.should include(event_on_day)
      
    end
    
    test "events on scope by month" do
      calendar = Calendar.make
      event_on_day = Event.make(:calendar => calendar, :starts_at => Time.parse( 'Thu Jan 01 00:00:01 UTC 1970'), :ends_at => Time.parse('Thu Jan 01 00:01:00 UTC 1970'))
      event_later = Event.make(:calendar => calendar, :starts_at => Time.parse( 'Thu Jan 02 00:00:01 UTC 1970'), :ends_at => Time.parse('Thu Jan 02 00:01:00 UTC 1970'))
      event_much_later = Event.make(:calendar => calendar, :starts_at => Time.parse( 'Thu Jan 02 00:00:01 UTC 1971'), :ends_at => Time.parse('Thu Jan 02 00:01:00 UTC 1971'))

      events_in_month = calendar.events_in_month(Date.civil(1970,1))
      events_in_month.should include(event_on_day)
      events_in_month.should include(event_later)
      events_in_month.should_not include(event_much_later)
    end
  end
end
