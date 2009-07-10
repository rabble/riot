require File.dirname(__FILE__) + '/../test_helper'


class CalendarsControllerTest < ActionController::TestCase

  describe_requests do
    setup = lambda do
      #stub(@controller).current_user { admin_user }
    end

    context "GET /calendars/1" do
      before(&setup)
      
      act! { get :show, {:id => Calendar.make.id} }
      
      it_renders :template, :show
      it_assigns :calendar
      it_assigns :month
      it_assigns :day
      it_assigns :year
    end
    
    context "calendar with events" do
      before do
        @calendar = Calendar.make
        @event = @calendar.events.make(:starts_at => Time.now, :ends_at => Time.now + 1.hour)
      end
      
      it "should be able to find the event thats happening this month" do
        get :show, {:id => @calendar.id}
        
        assigns(:calendar).events_in_month(assigns(:date)).should include(@event)
      end
    end
    
    context "GET /calendar/1/2009/12/20" do
      
      act! { get :day, {:id => Calendar.make.id, "month"=>"7", "id"=>"1", "day"=>"17", "year"=>"2009"} }
      
      it_renders :template, :day
      it_assigns :calendar
      it_assigns :month
      it_assigns :day
      it_assigns :year
      
    end
  end
end
