require File.dirname(__FILE__) + '/../test_helper'
require 'ruby-debug'

class EventTest < ActiveSupport::TestCase

  context "epoch and time" do
    
    test "can read" do
      event = Event.make(:start_epoch => 1, :end_epoch => 60)
      event.start_datetime.to_s.should == 'Thu Jan 01 00:00:01 UTC 1970'
      event.end_datetime.to_s.should == 'Thu Jan 01 00:01:00 UTC 1970'
    end
    
    test "can read with timezone offset" do
      event = Event.make(:start_epoch => 1, :end_epoch => 60, :timezone => '-0300')
      event.start_datetime.to_s.should == 'Wed Dec 31 23:00:01 -0300 1969'
      event.end_datetime.to_s.should == 'Wed Dec 31 23:01:00 -0300 1969'
    end
    
  end
end
