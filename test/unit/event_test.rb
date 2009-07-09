require File.dirname(__FILE__) + '/../test_helper'

class EventTest < ActiveSupport::TestCase

  context "epoch and time" do
    
    test "can read" do
      event = Event.make(:starts_at => Time.parse( 'Thu Jan 01 00:00:01 UTC 1970'), :ends_at => Time.parse('Thu Jan 01 00:01:00 UTC 1970'))

      event.starts_at.should be_kind_of(ActiveSupport::TimeWithZone)
      event.ends_at.should be_kind_of(ActiveSupport::TimeWithZone)

      event.starts_at_local.to_s.should == "1970-01-01 00:00:01 UTC"
      event.ends_at_local.to_s.should == "1970-01-01 00:01:00 UTC"
    end
    
    test "can read with timezone offset" do
      event = Event.make(:starts_at => Time.parse( 'Thu Jan 01 00:00:01 -0300 1970'), :ends_at => Time.parse('Thu Jan 01 00:01:00 -03:00 1970'), :timezone => "America/Montevideo")
      
      event.starts_at.should be_kind_of(ActiveSupport::TimeWithZone)
      event.ends_at.should be_kind_of(ActiveSupport::TimeWithZone)
      event.starts_at_local.utc_offset.should == -10800
      #event.start_local.should == "1970-01-01 00:00:01 -0300"
      #event.starts_at.in_time_zone.should == 'Thu Jan 01 00:00:01 -0300 1970'
    end
    
    test "sets timezone correctly when given lat / long" do
      event = Event.make(:latitude => 37, :longitude => -121, :starts_at => Time.parse( 'Thu Jan 01 00:00:01 1970'), :ends_at => Time.parse('Thu Jan 01 00:01:00 1970'))
      
      event.timezone.should == "America/Los_Angeles"
      
    end
    
  end
  
  context "event scopes" do
    
    test "events_on_day" do
      event_on_day = Event.make(:starts_at => Time.parse( 'Thu Jan 01 00:00:01 UTC 1970'), :ends_at => Time.parse('Thu Jan 01 00:01:00 UTC 1970'))
      event_later = Event.make(:starts_at => Time.parse( 'Thu Jan 02 00:00:01 UTC 1970'), :ends_at => Time.parse('Thu Jan 02 00:01:00 UTC 1970'))
      
      events = Event.on_day(Date.civil(1970,1,1))
      
      events.should include(event_on_day)
      
    end
  end
  
end
